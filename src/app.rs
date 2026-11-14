use std::io::{self, Write};
use std::path::Path;

use crate::AppError;
use crate::cli::{self, PatternSource};
use crate::discovery::walker;
use crate::input::patterns as pattern_input;
use crate::input::reader;
use crate::output::{OutputMode, TerminalOptions, terminal};
use crate::search::context::ContextSelector;
use crate::search::{LineMatch, PatternMatcher, SearchLine};

#[derive(Debug)]
pub enum SearchOutcome {
    Selected,
    NoSelection,
    Errors,
}

#[derive(Debug, Clone, Copy)]
struct FileSearchOptions {
    terminal: TerminalOptions,
    before_context: usize,
    after_context: usize,
    max_count: Option<usize>,
    quiet: bool,
    output_mode: OutputMode,
    heading: bool,
}

pub fn run() -> Result<SearchOutcome, AppError> {
    let config = cli::parse()?;

    let patterns = load_patterns(config.sources)?;

    let matcher = PatternMatcher::new(&patterns, config.pattern_options)
        .map_err(|error| AppError::Pattern(Box::new(error)))?;

    let options = FileSearchOptions {
        terminal: TerminalOptions {
            line_numbers: config.line_numbers,
            show_filename: config.show_filename,
            column: config.column,
            null: config.null,
        },
        before_context: config.before_context,
        after_context: config.after_context,
        max_count: config.max_count,
        quiet: config.quiet,
        output_mode: config.output_mode,
        heading: config.heading,
    };
    let mut remaining_results = config.max_results;

    if remaining_results == Some(0) {
        return Ok(SearchOutcome::NoSelection);
    }

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let stderr = io::stderr();
    let mut diagnostics = stderr.lock();

    let mut found_result = false;
    let mut had_errors = false;

    let mut headings = terminal::HeadingState::default();

    let candidates = walker::files(&config.path, &config.discovery)
        .map_err(|error| AppError::Discovery(Box::new(error)))?;

    for candidate in candidates {
        let result = candidate
            .map_err(|error| AppError::Discovery(Box::new(error)))
            .and_then(|path| {
                search_file(
                    &path,
                    &matcher,
                    options,
                    &mut remaining_results,
                    &mut headings,
                    &mut writer,
                )
            });

        match result {
            Ok(selected) => {
                found_result |= selected;
            }
            Err(error) => {
                if matches!(&error, AppError::Output(_)) {
                    return Err(error);
                }

                terminal::write_diagnostic(&mut diagnostics, &error).map_err(AppError::Output)?;

                had_errors = true;
            }
        }

        if remaining_results == Some(0) || (config.quiet && found_result) {
            break;
        }
    }

    if !config.quiet {
        writer.flush().map_err(AppError::Output)?;
    }
    diagnostics.flush().map_err(AppError::Output)?;

    if had_errors {
        Ok(SearchOutcome::Errors)
    } else if found_result {
        Ok(SearchOutcome::Selected)
    } else {
        Ok(SearchOutcome::NoSelection)
    }
}

fn search_file(
    path: &Path,
    matcher: &PatternMatcher,
    options: FileSearchOptions,
    remaining_results: &mut Option<usize>,
    headings: &mut terminal::HeadingState,
    writer: &mut impl Write,
) -> Result<bool, AppError> {
    let FileSearchOptions {
        terminal,
        before_context,
        after_context,
        max_count,
        quiet,
        output_mode,
        heading,
    } = options;

    if max_count == Some(0) || *remaining_results == Some(0) {
        return Ok(false);
    }

    let input = reader::open(path).map_err(|source| AppError::Open {
        path: path.to_path_buf(),
        source,
    })?;

    let heading_enabled =
        heading && !quiet && matches!(output_mode, OutputMode::Lines | OutputMode::OnlyMatching);

    if heading_enabled {
        headings.begin_file();
    }

    let terminal = TerminalOptions {
        show_filename: terminal.show_filename && !heading_enabled,
        ..terminal
    };

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

    for line in reader::numbered_lines(input) {
        let (line_number, text) = line.map_err(|source| AppError::Read {
            path: path.to_path_buf(),
            source,
        })?;

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
                    terminal::write_path(writer, path, terminal).map_err(AppError::Output)?;
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
                if heading_enabled {
                    headings
                        .before_record(writer, path)
                        .map_err(AppError::Output)?;
                }

                terminal::write_event(writer, path, &event, terminal).map_err(AppError::Output)?;
            }
        } else if selected {
            let matched_line = LineMatch {
                line_number,
                text: &text,
                spans: matcher.find_spans(&text),
            };

            if output_mode == OutputMode::OnlyMatching {
                let has_visible_match = matched_line.spans.iter().any(|span| !span.is_empty());

                if heading_enabled && has_visible_match {
                    headings
                        .before_record(writer, path)
                        .map_err(AppError::Output)?;
                }

                terminal::write_only_matching(writer, path, &matched_line, terminal)
                    .map_err(AppError::Output)?;
            } else {
                if heading_enabled {
                    headings
                        .before_record(writer, path)
                        .map_err(AppError::Output)?;
                }

                terminal::write_match(writer, path, &matched_line, terminal)
                    .map_err(AppError::Output)?;
            }
        }

        if trailing_remaining == Some(0) {
            break;
        }
    }

    if counting {
        if !quiet {
            terminal::write_count(writer, path, total, terminal).map_err(AppError::Output)?;
        }

        return Ok(found_selected_line);
    }

    if output_mode == OutputMode::FilesWithoutMatch {
        if !quiet {
            terminal::write_path(writer, path, terminal).map_err(AppError::Output)?;
        }

        return Ok(true);
    }

    Ok(found_selected_line)
}

fn load_patterns(sources: Vec<PatternSource>) -> Result<Vec<String>, AppError> {
    let mut patterns = Vec::new();

    for source in sources {
        match source {
            PatternSource::Argument(pattern) => patterns.push(pattern),
            PatternSource::File(path) => {
                let input = reader::open(&path).map_err(|source| AppError::Open {
                    path: path.clone(),
                    source,
                })?;

                let loaded = pattern_input::read(input).map_err(|source| AppError::Read {
                    path: path.clone(),
                    source,
                })?;

                patterns.extend(loaded);
            }
        }
    }

    Ok(patterns)
}
