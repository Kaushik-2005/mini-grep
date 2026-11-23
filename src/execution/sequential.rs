use std::io::BufRead;
use std::path::Path;

use super::stats::SearchStats;
use super::{ExecutionEvent, OutputMode, SearchOptions};
use crate::AppError;
use crate::input::reader;
use crate::search::context::ContextSelector;
use crate::search::{PatternMatcher, SearchEvent, SearchLine};

pub(crate) fn process_file(
    path: &Path,
    matcher: &PatternMatcher,
    options: SearchOptions<'_>,
    remaining_results: &mut Option<usize>,
    emit: &mut impl FnMut(ExecutionEvent) -> Result<(), AppError>,
) -> Result<bool, AppError> {
    options.cancellation.check()?;

    if options.max_count == Some(0) || *remaining_results == Some(0) {
        return Ok(false);
    }

    let input = reader::open(path).map_err(|source| AppError::Open {
        path: path.to_path_buf(),
        source,
    })?;

    process_reader(input, path, matcher, options, remaining_results, emit)
}

/// Opens and processes one file while preserving execution measurements.
///
/// An attempted open counts as a started input. Cancellation and zero limits
/// detected before opening do not count as started inputs.
pub(crate) fn process_file_with_stats(
    path: &Path,
    matcher: &PatternMatcher,
    options: SearchOptions<'_>,
    remaining_results: &mut Option<usize>,
    emit: &mut impl FnMut(ExecutionEvent) -> Result<(), AppError>,
    total: &mut SearchStats,
) -> Result<bool, AppError> {
    options.cancellation.check()?;

    if options.max_count == Some(0) || *remaining_results == Some(0) {
        return Ok(false);
    }

    let input = match reader::open(path) {
        Ok(input) => input,
        Err(source) => {
            total.merge(SearchStats {
                inputs_started: 1,
                errors: 1,
                ..SearchStats::default()
            });

            return Err(AppError::Open {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    process_reader_with_stats(
        input,
        path,
        matcher,
        options,
        remaining_results,
        emit,
        total,
    )
}

/// Processes one input without collecting statistics.
pub(crate) fn process_reader(
    input: impl BufRead,
    path: &Path,
    matcher: &PatternMatcher,
    options: SearchOptions<'_>,
    remaining_results: &mut Option<usize>,
    emit: &mut impl FnMut(ExecutionEvent) -> Result<(), AppError>,
) -> Result<bool, AppError> {
    process_reader_inner(input, path, matcher, options, remaining_results, emit, None)
}

/// Processes one input and merges its measurements, including partial progress.
///
/// Read errors are counted here. Output errors and cancellation are handled
/// by the application and are not classified as input read errors.
pub(crate) fn process_reader_with_stats(
    input: impl BufRead,
    path: &Path,
    matcher: &PatternMatcher,
    options: SearchOptions<'_>,
    remaining_results: &mut Option<usize>,
    emit: &mut impl FnMut(ExecutionEvent) -> Result<(), AppError>,
    total: &mut SearchStats,
) -> Result<bool, AppError> {
    let mut current = SearchStats::default();

    let result = process_reader_inner(
        input,
        path,
        matcher,
        options,
        remaining_results,
        emit,
        Some(&mut current),
    );

    if matches!(&result, Err(AppError::Read { .. })) {
        current.errors = current.errors.saturating_add(1);
    }

    total.merge(current);
    result
}

/// Processes one input and returns whether it qualifies for the requested mode.
/// A callback error stops processing immediately.
fn process_reader_inner(
    input: impl BufRead,
    path: &Path,
    matcher: &PatternMatcher,
    options: SearchOptions<'_>,
    remaining_results: &mut Option<usize>,
    emit: &mut impl FnMut(ExecutionEvent) -> Result<(), AppError>,
    mut stats: Option<&mut SearchStats>,
) -> Result<bool, AppError> {
    let SearchOptions {
        before_context,
        after_context,
        max_count,
        quiet,
        output_mode,
        cancellation,
        binary_policy,
    } = options;

    cancellation.check()?;

    if max_count == Some(0) || *remaining_results == Some(0) {
        return Ok(false);
    }

    if let Some(stats) = stats.as_deref_mut() {
        stats.inputs_started = 1;
    }

    emit(ExecutionEvent::BeginFile)?;

    let context_enabled =
        !quiet && output_mode == OutputMode::Lines && (before_context > 0 || after_context > 0);
    let mut context = ContextSelector::new(before_context, after_context);
    let mut found_selected_line = false;
    let mut remaining = max_count;
    let mut trailing_remaining: Option<usize> = None;

    let counting = matches!(
        output_mode,
        OutputMode::CountLines | OutputMode::CountMatches
    );
    let mut total: u128 = 0;

    let mut lines = reader::classified_lines(
        reader::CancellationReader::new(input, cancellation),
        binary_policy,
    );

    loop {
        cancellation.check()?;

        let Some(line) = lines.next() else {
            break;
        };

        cancellation.check()?;

        let input_line = line.map_err(|source| AppError::Read {
            path: path.to_path_buf(),
            source,
        })?;

        let (line_number, text) = match input_line {
            reader::InputLine::Text { line_number, text } => (line_number, text),
            reader::InputLine::Binary => {
                return Ok(output_mode != OutputMode::FilesWithoutMatch && found_selected_line);
            }
        };

        let selected = trailing_remaining.is_none() && matcher.is_selected(&text);
        found_selected_line |= selected;

        if selected {
            if let Some(stats) = stats.as_deref_mut() {
                stats.inputs_with_selected_lines = 1;
                stats.selected_lines = stats.selected_lines.saturating_add(1);
                stats.matches = stats
                    .matches
                    .saturating_add(matcher.find_spans(&text).len() as u128);
            }

            remaining = remaining.map(|count| count.saturating_sub(1));
            *remaining_results = (*remaining_results).map(|count| count.saturating_sub(1));

            if matches!(
                output_mode,
                OutputMode::FilesWithMatches | OutputMode::FilesWithoutMatch
            ) {
                let qualifies = output_mode == OutputMode::FilesWithMatches;

                if qualifies && !quiet {
                    emit(ExecutionEvent::Path)?;
                }

                return Ok(qualifies);
            }

            if quiet {
                return Ok(true);
            }

            if counting {
                total += match output_mode {
                    OutputMode::CountLines => 1,
                    OutputMode::CountMatches => matcher.find_spans(&text).len() as u128,
                    _ => unreachable!("counting requires a count output mode"),
                };

                if remaining == Some(0) || *remaining_results == Some(0) {
                    break;
                }

                lines.recycle(text);
                continue;
            }

            if remaining == Some(0) || *remaining_results == Some(0) {
                trailing_remaining = Some(if output_mode == OutputMode::OnlyMatching {
                    0
                } else {
                    after_context
                });
            }
        } else if let Some(count) = trailing_remaining.as_mut() {
            *count = count.saturating_sub(1);
        }

        if context_enabled {
            let spans = if selected {
                matcher.find_spans(&text)
            } else {
                Vec::new()
            };

            let line = SearchLine {
                line_number,
                text,
                spans,
                selected,
            };

            for event in context.push(line) {
                emit(ExecutionEvent::Search(event))?;
            }
        } else if selected {
            let spans = matcher.find_spans(&text);
            let line = SearchLine {
                line_number,
                text,
                spans,
                selected: true,
            };

            let event = if output_mode == OutputMode::OnlyMatching {
                ExecutionEvent::OnlyMatching(line)
            } else {
                ExecutionEvent::Search(SearchEvent::Line(line))
            };

            emit(event)?;
        } else {
            lines.recycle(text);
        }

        if trailing_remaining == Some(0) {
            break;
        }
    }

    cancellation.check()?;

    if counting {
        if !quiet {
            emit(ExecutionEvent::Count(total))?;
        }

        return Ok(found_selected_line);
    }

    if output_mode == OutputMode::FilesWithoutMatch {
        if !quiet {
            emit(ExecutionEvent::Path)?;
        }

        return Ok(true);
    }

    Ok(found_selected_line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    use crate::execution::cancel::CancellationToken;
    use crate::input::binary::BinaryPolicy;

    #[test]
    fn processor_stops_reading_when_event_consumer_fails() {
        let cancellation = CancellationToken::default();
        let patterns = vec![String::from("Rust")];
        let matcher =
            PatternMatcher::new(&patterns, Default::default()).expect("valid pattern must compile");

        let options = SearchOptions {
            before_context: 0,
            after_context: 0,
            max_count: None,
            quiet: false,
            output_mode: OutputMode::Lines,
            cancellation: &cancellation,
            binary_policy: BinaryPolicy::Skip,
        };

        let mut input = io::Cursor::new(b"Rust\nRust\n");
        let mut remaining_results = Some(2);
        let mut event_count = 0;

        let mut emit = |event| {
            event_count += 1;

            if matches!(event, ExecutionEvent::Search(_)) {
                Err(AppError::Output(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "consumer closed",
                )))
            } else {
                Ok(())
            }
        };

        let result = process_reader(
            &mut input,
            Path::new("sample.txt"),
            &matcher,
            options,
            &mut remaining_results,
            &mut emit,
        );

        assert!(matches!(
            result,
            Err(AppError::Output(source))
                if source.kind() == io::ErrorKind::BrokenPipe
        ));
        assert_eq!(input.position(), 5);
        assert_eq!(remaining_results, Some(1));
        assert_eq!(event_count, 2);
    }

    #[test]
    fn zero_limits_skip_file_opening_and_event_emission() {
        let cancellation = CancellationToken::default();
        let patterns = vec![String::from("Rust")];
        let matcher =
            PatternMatcher::new(&patterns, Default::default()).expect("valid pattern must compile");

        for (max_count, mut remaining_results) in [(Some(0), None), (None, Some(0))] {
            let options = SearchOptions {
                before_context: 0,
                after_context: 0,
                max_count,
                quiet: false,
                output_mode: OutputMode::Lines,
                cancellation: &cancellation,
                binary_policy: BinaryPolicy::Skip,
            };

            let mut event_count = 0;
            let mut emit = |_| {
                event_count += 1;
                Ok(())
            };

            let result = process_file(
                Path::new(""),
                &matcher,
                options,
                &mut remaining_results,
                &mut emit,
            )
            .expect("zero limits must skip opening the invalid empty path");

            assert!(!result);
            assert_eq!(event_count, 0);
        }
    }

    #[test]
    fn reader_stats_preserve_partial_progress_in_different_output_modes() {
        let cancellation = CancellationToken::default();
        let patterns = vec![String::from("Rust")];
        let matcher =
            PatternMatcher::new(&patterns, Default::default()).expect("valid pattern must compile");

        for output_mode in [
            OutputMode::Lines,
            OutputMode::CountLines,
            OutputMode::CountMatches,
            OutputMode::OnlyMatching,
        ] {
            for fail in [false, true] {
                let contents: &[u8] = if fail {
                    b"Rust Rust\nGo\nRust\n\xff\n"
                } else {
                    b"Rust Rust\nGo\nRust\n"
                };

                let options = SearchOptions {
                    before_context: 0,
                    after_context: 0,
                    max_count: None,
                    quiet: false,
                    output_mode,
                    cancellation: &cancellation,
                    binary_policy: BinaryPolicy::Skip,
                };

                let mut stats = SearchStats::default();
                let mut remaining_results = None;
                let mut emit = |_event: ExecutionEvent| Ok(());

                let result = process_reader_with_stats(
                    io::Cursor::new(contents),
                    Path::new("sample.txt"),
                    &matcher,
                    options,
                    &mut remaining_results,
                    &mut emit,
                    &mut stats,
                );

                if fail {
                    assert!(matches!(result, Err(AppError::Read { .. })));
                } else {
                    assert!(matches!(result, Ok(true)));
                }

                assert_eq!(
                    stats,
                    SearchStats {
                        inputs_started: 1,
                        inputs_with_selected_lines: 1,
                        selected_lines: 2,
                        matches: 3,
                        errors: if fail { 1 } else { 0 },
                    },
                    "mode: {output_mode:?}, read failure: {fail}"
                );
            }
        }
    }

    #[test]
    fn reader_stats_respect_limits_context_and_inversion() {
        use crate::search::{PatternOptions, Selection};

        let cancellation = CancellationToken::default();
        let patterns = vec![String::from("Rust")];

        for (
            name,
            selection,
            max_count,
            global_limit,
            expected_started,
            expected_lines,
            expected_matches,
        ) in [
            ("unlimited", Selection::Matching, None, None, 1, 3, 4),
            (
                "per-input limit",
                Selection::Matching,
                Some(1),
                None,
                1,
                1,
                2,
            ),
            ("global limit", Selection::Matching, None, Some(1), 1, 1, 2),
            ("inverted", Selection::NonMatching, None, None, 1, 1, 0),
            (
                "zero per-input limit",
                Selection::Matching,
                Some(0),
                None,
                0,
                0,
                0,
            ),
            (
                "zero global limit",
                Selection::Matching,
                None,
                Some(0),
                0,
                0,
                0,
            ),
        ] {
            let matcher = PatternMatcher::new(
                &patterns,
                PatternOptions {
                    selection,
                    ..PatternOptions::default()
                },
            )
            .expect("valid pattern must compile");

            let options = SearchOptions {
                before_context: 1,
                after_context: 1,
                max_count,
                quiet: false,
                output_mode: OutputMode::Lines,
                cancellation: &cancellation,
                binary_policy: BinaryPolicy::Skip,
            };

            let mut stats = SearchStats::default();
            let mut remaining_results = global_limit;
            let mut emit = |_event: ExecutionEvent| Ok(());

            let result = process_reader_with_stats(
                io::Cursor::new(b"Rust Rust\nRust\nGo\nRust\n"),
                Path::new("sample.txt"),
                &matcher,
                options,
                &mut remaining_results,
                &mut emit,
                &mut stats,
            );

            assert_eq!(
                result.expect("in-memory input must be readable"),
                expected_lines > 0,
                "{name}"
            );

            assert_eq!(
                stats,
                SearchStats {
                    inputs_started: expected_started,
                    inputs_with_selected_lines: u128::from(expected_lines > 0),
                    selected_lines: expected_lines,
                    matches: expected_matches,
                    errors: 0,
                },
                "{name}"
            );
        }
    }

    #[test]
    fn file_stats_count_open_failures_but_skip_zero_limits() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let cancellation = CancellationToken::default();
        let patterns = vec![String::from("Rust")];
        let matcher =
            PatternMatcher::new(&patterns, Default::default()).expect("valid pattern must compile");

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after the Unix epoch")
            .as_nanos();

        let missing = std::env::temp_dir()
            .join(format!(
                "mini-grep-missing-{}-{timestamp}",
                std::process::id()
            ))
            .join("missing.txt");

        for (max_count, global_limit, should_open) in [
            (None, None, true),
            (Some(0), None, false),
            (None, Some(0), false),
        ] {
            let options = SearchOptions {
                before_context: 0,
                after_context: 0,
                max_count,
                quiet: false,
                output_mode: OutputMode::Lines,
                cancellation: &cancellation,
                binary_policy: BinaryPolicy::Skip,
            };

            let mut stats = SearchStats::default();
            let mut remaining_results = global_limit;
            let mut event_count = 0;

            let mut emit = |_event: ExecutionEvent| {
                event_count += 1;
                Ok(())
            };

            let result = process_file_with_stats(
                &missing,
                &matcher,
                options,
                &mut remaining_results,
                &mut emit,
                &mut stats,
            );

            if should_open {
                assert!(matches!(result, Err(AppError::Open { .. })));
                assert_eq!(
                    stats,
                    SearchStats {
                        inputs_started: 1,
                        errors: 1,
                        ..SearchStats::default()
                    }
                );
            } else {
                assert!(matches!(result, Ok(false)));
                assert_eq!(stats, SearchStats::default());
            }

            assert_eq!(event_count, 0);
        }
    }
}
