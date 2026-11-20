use std::io::BufRead;
use std::path::Path;

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

/// Processes one input and returns whether it qualifies for the requested mode.
/// A callback error stops processing immediately.
pub(crate) fn process_reader(
    input: impl BufRead,
    path: &Path,
    matcher: &PatternMatcher,
    options: SearchOptions<'_>,
    remaining_results: &mut Option<usize>,
    emit: &mut impl FnMut(ExecutionEvent) -> Result<(), AppError>,
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
}
