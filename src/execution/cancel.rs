use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::AppError;

/// Clones share on cancellation request.
/// Separate default tokens have independent state.
#[derive(Debug, Clone, Default)]
pub(crate) struct CancellationToken {
    requested: Arc<AtomicBool>,
}

impl CancellationToken {
    pub(crate) fn cancel(&self) {
        self.requested.store(true, Ordering::Relaxed);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.requested.load(Ordering::Relaxed)
    }

    pub(crate) fn check(&self) -> Result<(), AppError> {
        if self.is_cancelled() {
            Err(AppError::Cancelled)
        } else {
            Ok(())
        }
    }

    pub(crate) fn install_handler(&self) -> Result<(), AppError> {
        let token = self.clone();
        let mut received_interrupt = false;

        ctrlc::try_set_handler(move || {
            if received_interrupt {
                // Emergency exit when cooperative cancellation cannot progress.
                std::process::exit(2);
            }

            received_interrupt = true;
            token.cancel();
        })
        .map_err(AppError::Signal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_without_a_cancellation_request() {
        let token = CancellationToken::default();

        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancellation_is_permanent_and_repeatable() {
        let token = CancellationToken::default();

        token.cancel();
        assert!(token.is_cancelled());

        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn clones_share_cancellation_across_threads() {
        let token = CancellationToken::default();
        let worker_token = token.clone();

        std::thread::spawn(move || {
            worker_token.cancel();
        })
        .join()
        .expect("cancellation thread must finish");

        assert!(token.is_cancelled());
    }

    #[test]
    fn separate_tokens_remain_independent() {
        let first = CancellationToken::default();
        let second = CancellationToken::default();

        first.cancel();

        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
    }

    #[test]
    fn check_reports_a_cancellation_request() {
        let token = CancellationToken::default();

        assert!(token.check().is_ok());

        token.cancel();

        assert!(matches!(token.check(), Err(AppError::Cancelled)));
    }
}
