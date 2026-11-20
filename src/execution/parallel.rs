use std::collections::VecDeque;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, bounded};

use super::cancel::CancellationToken;
use super::{ExecutionEvent, SearchOptions, sequential};
use crate::AppError;
use crate::search::PatternMatcher;

const JOB_QUEUE_CAPACITY: usize = 16;
const EVENT_QUEUE_CAPACITY: usize = 16;
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(crate) enum ParallelEvent<'a> {
    Output {
        path: &'a Path,
        event: ExecutionEvent,
    },
    Finished(Result<bool, AppError>),
    DiscoveryError(AppError),
}

enum WorkerMessage {
    Output(ExecutionEvent),
    Finished(Result<bool, AppError>),
}

struct Job {
    path: Arc<PathBuf>,
    output: Sender<WorkerMessage>,
}

struct Pending {
    path: Arc<PathBuf>,
    output: Receiver<WorkerMessage>,
}

fn worker_error(message: &'static str) -> AppError {
    AppError::Worker(io::Error::other(message))
}

pub(crate) fn run(
    candidates: impl Iterator<Item = Result<PathBuf, AppError>>,
    matcher: &PatternMatcher,
    options: SearchOptions<'_>,
    threads: NonZeroUsize,
    handle: &mut impl FnMut(ParallelEvent<'_>) -> Result<(), AppError>,
) -> Result<(), AppError> {
    thread::scope(|scope| {
        let (jobs_sender, jobs_receiver) = bounded(JOB_QUEUE_CAPACITY);
        let mut workers = Vec::new();

        let spawn_result: Result<(), AppError> = (|| {
            for index in 0..threads.get() {
                options.cancellation.check()?;

                let jobs = jobs_receiver.clone();
                let worker = thread::Builder::new()
                    .name(format!("mini-grep-worker-{index}"))
                    .spawn_scoped(scope, move || {
                        worker_loop(jobs, matcher, options);
                    })
                    .map_err(AppError::Worker)?;

                workers.push(worker);
            }

            Ok(())
        })();

        drop(jobs_receiver);

        let mut result = match spawn_result {
            Ok(()) => coordinate(
                candidates,
                &jobs_sender,
                threads.get(),
                options.cancellation,
                handle,
            ),
            Err(error) => Err(error),
        };

        if result.is_err() {
            options.cancellation.cancel();
        }

        drop(jobs_sender);

        for worker in workers {
            if worker.join().is_err() && result.is_ok() {
                result = Err(worker_error("search worker panicked"));
            }
        }

        result
    })
}

fn coordinate(
    mut candidates: impl Iterator<Item = Result<PathBuf, AppError>>,
    jobs: &Sender<Job>,
    window: usize,
    cancellation: &CancellationToken,
    handle: &mut impl FnMut(ParallelEvent<'_>) -> Result<(), AppError>,
) -> Result<(), AppError> {
    let mut pending = VecDeque::new();
    let mut exhausted = false;

    loop {
        while !exhausted && pending.len() < window {
            cancellation.check()?;
            let candidate = candidates.next();
            cancellation.check()?;

            let path = match candidate {
                None => {
                    exhausted = true;
                    break;
                }
                Some(Err(error)) => {
                    handle(ParallelEvent::DiscoveryError(error))?;
                    continue;
                }
                Some(Ok(path)) => Arc::new(path),
            };

            let (output_sender, output_receiver) = bounded(EVENT_QUEUE_CAPACITY);

            let submitted = jobs.send(Job {
                path: Arc::clone(&path),
                output: output_sender,
            });

            cancellation.check()?;

            submitted.map_err(|_| worker_error("search workers disconnected"))?;

            pending.push_back(Pending {
                path,
                output: output_receiver,
            });
        }

        let Some(current) = pending.pop_front() else {
            return Ok(());
        };

        loop {
            cancellation.check()?;
            let message = current.output.recv_timeout(CANCEL_POLL_INTERVAL);
            cancellation.check()?;

            match message {
                Ok(WorkerMessage::Output(event)) => {
                    handle(ParallelEvent::Output {
                        path: current.path.as_path(),
                        event,
                    })?;
                }
                Ok(WorkerMessage::Finished(result)) => {
                    handle(ParallelEvent::Finished(result))?;
                    break;
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(worker_error(
                        "search worker disconnected before reporting completion",
                    ));
                }
            }
        }
    }
}

fn worker_loop(jobs: Receiver<Job>, matcher: &PatternMatcher, options: SearchOptions<'_>) {
    while let Ok(job) = jobs.recv() {
        if options.cancellation.is_cancelled() {
            break;
        }

        let mut remaining_results = None;

        let mut emit = |event| {
            job.output
                .send(WorkerMessage::Output(event))
                .map_err(|_| AppError::Cancelled)
        };

        let result = sequential::process_file(
            job.path.as_path(),
            matcher,
            options,
            &mut remaining_results,
            &mut emit,
        );

        // A closed receiver means the coordinator has stopped this work.
        let _ = job.output.send(WorkerMessage::Finished(result));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::Cell;

    use crossbeam_channel::SendTimeoutError;

    fn unexpected_event(_: ParallelEvent<'_>) -> Result<(), AppError> {
        panic!("no output or completion event was expected");
    }

    #[test]
    fn disconnected_job_receivers_produce_a_worker_error() {
        let cancellation = CancellationToken::default();
        let (jobs_sender, jobs_receiver) = bounded(1);
        drop(jobs_receiver);

        let candidates = std::iter::once(Ok(PathBuf::from("unused.txt")));
        let mut handle = unexpected_event;

        let result = coordinate(candidates, &jobs_sender, 1, &cancellation, &mut handle);

        assert!(matches!(result, Err(AppError::Worker(_))));
    }

    #[test]
    fn missing_worker_completion_produces_a_worker_error() {
        let cancellation = CancellationToken::default();
        let (jobs_sender, jobs_receiver) = bounded(1);

        let worker = thread::spawn(move || {
            let job = jobs_receiver.recv().expect("job must arrive");
            drop(job);
        });

        let candidates = std::iter::once(Ok(PathBuf::from("unused.txt")));
        let mut handle = unexpected_event;

        let result = coordinate(candidates, &jobs_sender, 1, &cancellation, &mut handle);

        worker.join().expect("test worker must finish");

        assert!(matches!(result, Err(AppError::Worker(_))));
    }

    #[test]
    fn cancellation_wakes_coordinator_while_worker_output_remains_open() {
        let cancellation = CancellationToken::default();
        let worker_cancellation = cancellation.clone();

        let (jobs_sender, jobs_receiver) = bounded(1);
        let (release_sender, release_receiver) = bounded(1);

        let worker = thread::spawn(move || {
            let job = jobs_receiver.recv().expect("job must arrive");
            worker_cancellation.cancel();

            let released = release_receiver
                .recv_timeout(Duration::from_secs(5))
                .is_ok();

            drop(job);
            released
        });

        let candidates = std::iter::once(Ok(PathBuf::from("unused.txt")));
        let mut handle = unexpected_event;

        let result = coordinate(candidates, &jobs_sender, 1, &cancellation, &mut handle);

        let release_sent = release_sender.send(()).is_ok();
        let worker_released = worker.join().expect("test worker must finish");

        assert!(release_sent);
        assert!(
            worker_released,
            "coordinator must observe cancellation before worker disconnection",
        );
        assert!(matches!(result, Err(AppError::Cancelled)));
    }

    #[test]
    fn active_window_bounds_candidate_discovery() {
        let cancellation = CancellationToken::default();
        let pulled = Cell::new(0);
        let (jobs_sender, jobs_receiver) = bounded::<Job>(2);

        let worker = thread::spawn(move || {
            while let Ok(job) = jobs_receiver.recv() {
                let _ = job
                    .output
                    .send(WorkerMessage::Output(ExecutionEvent::BeginFile));
                let _ = job.output.send(WorkerMessage::Finished(Ok(false)));
            }
        });

        let candidates = (0..100).map(|index| {
            pulled.set(pulled.get() + 1);
            Ok(PathBuf::from(format!("input-{index}.txt")))
        });

        let mut observed_pulls = 0;
        let mut handle = |_: ParallelEvent<'_>| {
            observed_pulls = pulled.get();

            Err(AppError::Output(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "consumer stopped",
            )))
        };

        let result = coordinate(candidates, &jobs_sender, 2, &cancellation, &mut handle);

        drop(jobs_sender);
        worker.join().expect("test worker must finish");

        assert!(matches!(result, Err(AppError::Output(_))));
        assert_eq!(observed_pulls, 2);
        assert_eq!(pulled.get(), 2);
    }

    #[test]
    fn paused_consumer_applies_event_queue_backpressure() {
        let cancellation = CancellationToken::default();
        let (jobs_sender, jobs_receiver) = bounded::<Job>(1);
        let (resume_sender, resume_receiver) = bounded::<()>(1);
        let (report_sender, report_receiver) = bounded::<(usize, bool)>(1);

        let worker = thread::spawn(move || {
            let job = jobs_receiver.recv().expect("job must arrive");

            job.output
                .send(WorkerMessage::Output(ExecutionEvent::Count(0)))
                .expect("first event must reach the coordinator");

            resume_receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("consumer must acknowledge the first event");

            let mut successful_sends = 1;
            let mut backpressure_observed = false;

            for index in 1..EVENT_QUEUE_CAPACITY + 2 {
                let message = WorkerMessage::Output(ExecutionEvent::Count(index as u128));

                match job.output.send_timeout(message, Duration::from_millis(100)) {
                    Ok(()) => successful_sends += 1,
                    Err(SendTimeoutError::Timeout(_)) => {
                        backpressure_observed = true;
                        break;
                    }
                    Err(SendTimeoutError::Disconnected(_)) => break,
                }
            }

            let _ = report_sender.send((successful_sends, backpressure_observed));
        });

        let candidates = std::iter::once(Ok(PathBuf::from("unused.txt")));
        let mut observed = None;

        let mut handle = |_: ParallelEvent<'_>| {
            resume_sender
                .send(())
                .expect("producer must await the consumer acknowledgement");

            observed = Some(
                report_receiver
                    .recv_timeout(Duration::from_secs(5))
                    .expect("producer must report queue behavior"),
            );

            Err(AppError::Output(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "paused consumer stopped",
            )))
        };

        let result = coordinate(candidates, &jobs_sender, 1, &cancellation, &mut handle);

        drop(jobs_sender);
        worker.join().expect("test worker must finish");

        assert!(matches!(result, Err(AppError::Output(_))));
        assert_eq!(observed, Some((EVENT_QUEUE_CAPACITY + 1, true)));
    }
}
