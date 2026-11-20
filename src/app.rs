use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::vec;

use crate::AppError;
use crate::cli::{self, PathListSource, PatternSource};
use crate::discovery::{DiscoveryOptions, walker};
use crate::execution::cancel::CancellationToken;
use crate::execution::{OutputMode, SearchOptions, ordering, parallel, sequential};
use crate::input::paths as path_input;
use crate::input::patterns as pattern_input;
use crate::input::reader;
use crate::input::stdin;
use crate::output::{TerminalOptions, color, terminal};
use crate::search::PatternMatcher;

#[derive(Debug)]
pub enum SearchOutcome {
    Selected,
    NoSelection,
    Errors,
}

#[derive(Debug, Clone, Copy)]
struct FileSearchOptions<'a> {
    search: SearchOptions<'a>,
    terminal: TerminalOptions,
    heading: bool,
}

pub fn run() -> Result<SearchOutcome, AppError> {
    let config = cli::parse()?;

    let cancellation = CancellationToken::default();
    cancellation.install_handler()?;
    cancellation.check()?;

    if config.files {
        let roots = if config.path_lists.is_empty() {
            vec![config.path.unwrap_or_else(|| PathBuf::from("."))]
        } else {
            load_path_lists(config.path_lists, &cancellation)?
        };

        return list_files(
            &roots,
            &config.discovery,
            config.sort,
            TerminalOptions {
                null: config.null,
                ..TerminalOptions::default()
            },
            config.quiet,
            &cancellation,
        );
    }

    let patterns_from_stdin = config
        .sources
        .iter()
        .any(|source| matches!(source, PatternSource::Stdin));

    let patterns = load_patterns(config.sources, &cancellation)?;

    let matcher = PatternMatcher::new(&patterns, config.pattern_options)
        .map_err(|error| AppError::Pattern(Box::new(error)))?;
    cancellation.check()?;

    let listed_roots = if config.path_lists.is_empty() {
        None
    } else {
        Some(load_path_lists(config.path_lists, &cancellation)?)
    };

    let options = FileSearchOptions {
        search: SearchOptions {
            before_context: config.before_context,
            after_context: config.after_context,
            max_count: config.max_count,
            quiet: config.quiet,
            output_mode: config.output_mode,
            cancellation: &cancellation,
            binary_policy: config.binary_policy,
        },
        terminal: TerminalOptions {
            line_numbers: config.line_numbers,
            show_filename: config.show_filename,
            column: config.column,
            null: config.null,
            color: config.color.enabled(io::stdout().is_terminal()),
        },
        heading: config.heading,
    };
    let mut remaining_results = config.max_results;

    cancellation.check()?;

    if remaining_results == Some(0) {
        return Ok(SearchOutcome::NoSelection);
    }

    let (roots, use_stdin) = match listed_roots {
        Some(roots) => (roots, false),
        None => {
            let path = if patterns_from_stdin {
                config.path.unwrap_or_else(|| PathBuf::from("."))
            } else {
                stdin::resolve_target(config.path, io::stdin().is_terminal())
            };

            let use_stdin = path.as_os_str() == "-";
            (vec![path], use_stdin)
        }
    };

    let mut writer = color::stdout(config.color);
    let stderr = io::stderr();
    let mut diagnostics = stderr.lock();

    let mut found_result = false;
    let mut had_errors = false;

    let mut headings = terminal::HeadingState::default();

    let candidates: Box<dyn Iterator<Item = Result<PathBuf, AppError>> + '_> = if use_stdin {
        Box::new(std::iter::empty())
    } else {
        let mut report = |error: AppError| {
            terminal::write_diagnostic(&mut diagnostics, &error).map_err(AppError::Diagnostic)?;
            had_errors = true;
            Ok(())
        };

        ordering::prepare(
            discover_candidates(&roots, &config.discovery),
            config.sort,
            &cancellation,
            &mut report,
        )?
    };

    if use_stdin {
        let result = search_reader(
            stdin::lock(),
            Path::new(stdin::LABEL),
            &matcher,
            options,
            &mut remaining_results,
            &mut headings,
            &mut writer,
        );

        match result {
            Ok(selected) => {
                found_result = selected;
            }
            Err(error) => {
                if matches!(&error, AppError::Cancelled) {
                    return Err(error);
                }

                if let AppError::Output(source) = error {
                    return finish_output_error(source, had_errors, &mut diagnostics);
                }

                terminal::write_diagnostic(&mut diagnostics, &error)
                    .map_err(AppError::Diagnostic)?;
                had_errors = true;
            }
        }
    } else if config.threads.get() > 1 && !config.quiet && remaining_results.is_none() {
        let heading_enabled = options.heading
            && matches!(
                options.search.output_mode,
                OutputMode::Lines | OutputMode::OnlyMatching
            );

        let result = {
            let mut handle = |event: parallel::ParallelEvent<'_>| {
                match event {
                    parallel::ParallelEvent::Output { path, event } => {
                        terminal::write_execution_event(
                            &mut writer,
                            path,
                            event,
                            options.terminal,
                            heading_enabled,
                            &mut headings,
                        )
                        .map_err(AppError::Output)?;
                    }
                    parallel::ParallelEvent::Finished(Ok(selected)) => {
                        found_result |= selected;
                    }
                    parallel::ParallelEvent::Finished(Err(error))
                    | parallel::ParallelEvent::DiscoveryError(error) => {
                        if matches!(&error, AppError::Cancelled) {
                            return Err(error);
                        }

                        terminal::write_diagnostic(&mut diagnostics, &error)
                            .map_err(AppError::Diagnostic)?;
                        had_errors = true;
                    }
                }

                Ok(())
            };

            parallel::run(
                candidates,
                &matcher,
                options.search,
                config.threads,
                &mut handle,
            )
        };

        if let Err(error) = result {
            if let AppError::Output(source) = error {
                return finish_output_error(source, had_errors, &mut diagnostics);
            }

            return Err(error);
        }
    } else {
        let mut candidates = candidates;

        loop {
            cancellation.check()?;

            let Some(candidate) = candidates.next() else {
                break;
            };

            cancellation.check()?;

            let result = candidate.and_then(|path| {
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
                    if matches!(&error, AppError::Cancelled) {
                        return Err(error);
                    }

                    if let AppError::Output(source) = error {
                        return finish_output_error(source, had_errors, &mut diagnostics);
                    }

                    terminal::write_diagnostic(&mut diagnostics, &error)
                        .map_err(AppError::Diagnostic)?;
                    had_errors = true;
                }
            }

            if remaining_results == Some(0) || (config.quiet && found_result) {
                break;
            }
        }
    }

    cancellation.check()?;

    if !config.quiet
        && let Err(source) = writer.flush()
    {
        return finish_output_error(source, had_errors, &mut diagnostics);
    }
    diagnostics.flush().map_err(AppError::Diagnostic)?;

    if had_errors {
        Ok(SearchOutcome::Errors)
    } else if found_result {
        Ok(SearchOutcome::Selected)
    } else {
        Ok(SearchOutcome::NoSelection)
    }
}

fn discover_candidates<'a>(
    roots: &'a [PathBuf],
    options: &'a DiscoveryOptions,
) -> impl Iterator<Item = Result<PathBuf, AppError>> + 'a {
    roots.iter().flat_map(move |root| {
        let candidates: Box<dyn Iterator<Item = Result<PathBuf, AppError>> + 'a> =
            match walker::files(root, options) {
                Ok(candidates) => Box::new(
                    candidates
                        .map(|result| result.map_err(|error| AppError::Discovery(Box::new(error)))),
                ),
                Err(error) => Box::new(std::iter::once(Err(AppError::Discovery(Box::new(error))))),
            };

        candidates
    })
}

fn finish_output_error(
    source: io::Error,
    had_errors: bool,
    diagnostics: &mut impl Write,
) -> Result<SearchOutcome, AppError> {
    if source.kind() != io::ErrorKind::BrokenPipe {
        return Err(AppError::Output(source));
    }

    diagnostics.flush().map_err(AppError::Diagnostic)?;

    if had_errors {
        Ok(SearchOutcome::Errors)
    } else {
        Ok(SearchOutcome::Selected)
    }
}

fn list_files(
    roots: &[PathBuf],
    discovery: &DiscoveryOptions,
    order: ordering::SortOrder,
    options: TerminalOptions,
    quiet: bool,
    cancellation: &CancellationToken,
) -> Result<SearchOutcome, AppError> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let stderr = io::stderr();
    let mut diagnostics = stderr.lock();

    let mut found_file = false;
    let mut had_errors = false;

    let mut candidates = {
        let mut report = |error: AppError| {
            terminal::write_diagnostic(&mut diagnostics, &error).map_err(AppError::Diagnostic)?;
            had_errors = true;
            Ok(())
        };

        ordering::prepare(
            discover_candidates(roots, discovery),
            order,
            cancellation,
            &mut report,
        )?
    };

    loop {
        cancellation.check()?;

        let Some(candidate) = candidates.next() else {
            break;
        };

        cancellation.check()?;

        match candidate {
            Ok(path) => {
                found_file = true;

                if quiet {
                    break;
                }

                if let Err(source) = terminal::write_path(&mut writer, &path, options) {
                    return finish_output_error(source, had_errors, &mut diagnostics);
                }
            }
            Err(error) => {
                terminal::write_diagnostic(&mut diagnostics, &error)
                    .map_err(AppError::Diagnostic)?;
                had_errors = true;
            }
        }
    }

    cancellation.check()?;

    if !quiet && let Err(source) = writer.flush() {
        return finish_output_error(source, had_errors, &mut diagnostics);
    }
    diagnostics.flush().map_err(AppError::Diagnostic)?;

    if had_errors {
        Ok(SearchOutcome::Errors)
    } else if found_file {
        Ok(SearchOutcome::Selected)
    } else {
        Ok(SearchOutcome::NoSelection)
    }
}

fn search_file(
    path: &Path,
    matcher: &PatternMatcher,
    options: FileSearchOptions<'_>,
    remaining_results: &mut Option<usize>,
    headings: &mut terminal::HeadingState,
    writer: &mut impl Write,
) -> Result<bool, AppError> {
    let FileSearchOptions {
        search,
        terminal,
        heading,
    } = options;

    let heading_enabled = heading
        && !search.quiet
        && matches!(
            search.output_mode,
            OutputMode::Lines | OutputMode::OnlyMatching
        );

    let mut emit = |event| {
        terminal::write_execution_event(writer, path, event, terminal, heading_enabled, headings)
            .map_err(AppError::Output)
    };

    sequential::process_file(path, matcher, search, remaining_results, &mut emit)
}

fn search_reader(
    input: impl BufRead,
    path: &Path,
    matcher: &PatternMatcher,
    options: FileSearchOptions<'_>,
    remaining_results: &mut Option<usize>,
    headings: &mut terminal::HeadingState,
    writer: &mut impl Write,
) -> Result<bool, AppError> {
    let FileSearchOptions {
        search,
        terminal,
        heading,
    } = options;

    let heading_enabled = heading
        && !search.quiet
        && matches!(
            search.output_mode,
            OutputMode::Lines | OutputMode::OnlyMatching
        );

    let mut emit = |event| {
        terminal::write_execution_event(writer, path, event, terminal, heading_enabled, headings)
            .map_err(AppError::Output)
    };

    sequential::process_reader(input, path, matcher, search, remaining_results, &mut emit)
}

fn load_path_lists(
    sources: Vec<PathListSource>,
    cancellation: &CancellationToken,
) -> Result<Vec<PathBuf>, AppError> {
    let mut paths = Vec::new();

    for source in sources {
        cancellation.check()?;

        let (path, result) = match source {
            PathListSource::File(path) => {
                let input = reader::open(&path).map_err(|source| AppError::Open {
                    path: path.clone(),
                    source,
                })?;

                let result = path_input::read(reader::CancellationReader::new(input, cancellation));

                (path, result)
            }
            PathListSource::Stdin => {
                let result =
                    path_input::read(reader::CancellationReader::new(stdin::lock(), cancellation));

                (PathBuf::from(stdin::LABEL), result)
            }
        };

        cancellation.check()?;

        let loaded = result.map_err(|source| AppError::Read { path, source })?;
        paths.extend(loaded);
    }

    cancellation.check()?;
    Ok(paths)
}

fn load_patterns(
    sources: Vec<PatternSource>,
    cancellation: &CancellationToken,
) -> Result<Vec<String>, AppError> {
    let mut patterns = Vec::new();

    for source in sources {
        cancellation.check()?;

        let (path, result) = match source {
            PatternSource::Argument(pattern) => {
                patterns.push(pattern);
                continue;
            }
            PatternSource::File(path) => {
                let input = reader::open(&path).map_err(|source| AppError::Open {
                    path: path.clone(),
                    source,
                })?;

                let result =
                    pattern_input::read(reader::CancellationReader::new(input, cancellation));

                (path, result)
            }
            PatternSource::Stdin => {
                let result = pattern_input::read(reader::CancellationReader::new(
                    stdin::lock(),
                    cancellation,
                ));

                (PathBuf::from(stdin::LABEL), result)
            }
        };

        cancellation.check()?;

        let loaded = result.map_err(|source| AppError::Read { path, source })?;
        patterns.extend(loaded);
    }

    cancellation.check()?;
    Ok(patterns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::binary::BinaryPolicy;

    #[test]
    fn broken_stdout_preserves_earlier_error_status() {
        for had_errors in [false, true] {
            let mut diagnostics = Vec::new();

            let outcome = finish_output_error(
                io::Error::new(io::ErrorKind::BrokenPipe, "consumer closed"),
                had_errors,
                &mut diagnostics,
            )
            .expect("a broken stdout pipe must stop gracefully");

            if had_errors {
                assert!(matches!(outcome, SearchOutcome::Errors));
            } else {
                assert!(matches!(outcome, SearchOutcome::Selected));
            }

            assert!(diagnostics.is_empty());
        }
    }

    #[test]
    fn other_stdout_failures_remain_errors() {
        let mut diagnostics = Vec::new();

        let error = finish_output_error(
            io::Error::new(io::ErrorKind::PermissionDenied, "output denied"),
            false,
            &mut diagnostics,
        )
        .expect_err("ordinary stdout failures must remain errors");

        match error {
            AppError::Output(source) => {
                assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
            }
            other => panic!("expected stdout error, got {other:?}"),
        }
    }

    struct FailingDiagnosticFlush;

    impl Write for FailingDiagnosticFlush {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "diagnostic consumer closed",
            ))
        }
    }

    #[test]
    fn broken_stdout_does_not_hide_a_diagnostic_flush_failure() {
        let error = finish_output_error(
            io::Error::new(io::ErrorKind::BrokenPipe, "consumer closed"),
            false,
            &mut FailingDiagnosticFlush,
        )
        .expect_err("stderr flush failures must remain errors");

        match error {
            AppError::Diagnostic(source) => {
                assert_eq!(source.kind(), io::ErrorKind::BrokenPipe);
            }
            other => panic!("expected diagnostic error, got {other:?}"),
        }
    }

    #[test]
    fn cancelled_search_does_not_read_or_write() {
        let cancellation = CancellationToken::default();
        cancellation.cancel();

        let patterns = vec![String::from("Rust")];
        let matcher =
            PatternMatcher::new(&patterns, Default::default()).expect("valid pattern must compile");

        let options = FileSearchOptions {
            search: SearchOptions {
                cancellation: &cancellation,
                before_context: 0,
                after_context: 0,
                max_count: None,
                quiet: false,
                output_mode: OutputMode::Lines,
                binary_policy: BinaryPolicy::Skip,
            },
            terminal: TerminalOptions::default(),
            heading: false,
        };

        let mut input = io::Cursor::new(b"Rust\n");
        let mut remaining_results = None;
        let mut headings = terminal::HeadingState::default();
        let mut output = Vec::new();

        let result = search_reader(
            &mut input,
            Path::new("sample.txt"),
            &matcher,
            options,
            &mut remaining_results,
            &mut headings,
            &mut output,
        );

        assert!(matches!(result, Err(AppError::Cancelled)));
        assert_eq!(input.position(), 0);
        assert!(output.is_empty());
    }

    #[test]
    fn cancelled_pattern_loading_does_not_open_sources() {
        let cancellation = CancellationToken::default();
        cancellation.cancel();

        let sources = vec![PatternSource::File(PathBuf::from(
            "missing-pattern-source.txt",
        ))];

        assert!(matches!(
            load_patterns(sources, &cancellation),
            Err(AppError::Cancelled)
        ));
        assert!(matches!(
            load_patterns(Vec::new(), &cancellation),
            Err(AppError::Cancelled)
        ));
    }

    #[test]
    fn cancelled_path_loading_does_not_open_sources() {
        let cancellation = CancellationToken::default();
        cancellation.cancel();

        let sources = vec![PathListSource::File(PathBuf::from(
            "missing-path-source.txt",
        ))];

        assert!(matches!(
            load_path_lists(sources, &cancellation),
            Err(AppError::Cancelled)
        ));
        assert!(matches!(
            load_path_lists(Vec::new(), &cancellation),
            Err(AppError::Cancelled)
        ));
    }

    struct CancelAtEof<'a> {
        input: io::Cursor<&'static [u8]>,
        cancellation: &'a CancellationToken,
    }

    impl io::Read for CancelAtEof<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if buffer.is_empty() {
                return Ok(0);
            }

            let amount = io::Read::read(&mut self.input, buffer)?;

            if amount == 0 {
                self.cancellation.cancel();
            }

            Ok(amount)
        }
    }

    impl BufRead for CancelAtEof<'_> {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            let bytes = self.input.fill_buf()?;

            if bytes.is_empty() {
                self.cancellation.cancel();
            }

            Ok(bytes)
        }

        fn consume(&mut self, amount: usize) {
            self.input.consume(amount);
        }
    }

    #[test]
    fn cancellation_at_eof_preserves_lines_but_not_completion_records() {
        let cases: &[(OutputMode, &'static [u8], &[u8])] = &[
            (OutputMode::Lines, b"Rust\n", b"sample.txt:Rust\n"),
            (OutputMode::CountLines, b"Rust\n", b""),
            (OutputMode::CountMatches, b"Rust\n", b""),
            (OutputMode::FilesWithoutMatch, b"Go\n", b""),
        ];

        let patterns = vec![String::from("Rust")];
        let matcher =
            PatternMatcher::new(&patterns, Default::default()).expect("valid pattern must compile");

        for &(output_mode, contents, expected_output) in cases {
            let cancellation = CancellationToken::default();

            let input = CancelAtEof {
                input: io::Cursor::new(contents),
                cancellation: &cancellation,
            };

            let options = FileSearchOptions {
                search: SearchOptions {
                    before_context: 0,
                    after_context: 0,
                    max_count: None,
                    quiet: false,
                    output_mode,
                    cancellation: &cancellation,
                    binary_policy: BinaryPolicy::Skip,
                },
                terminal: TerminalOptions::default(),
                heading: false,
            };

            let mut remaining_results = None;
            let mut headings = terminal::HeadingState::default();
            let mut output = Vec::new();

            let result = search_reader(
                input,
                Path::new("sample.txt"),
                &matcher,
                options,
                &mut remaining_results,
                &mut headings,
                &mut output,
            );

            assert!(
                matches!(result, Err(AppError::Cancelled)),
                "mode: {output_mode:?}; result: {result:?}",
            );
            assert_eq!(output, expected_output, "mode: {output_mode:?}");
        }
    }
}
