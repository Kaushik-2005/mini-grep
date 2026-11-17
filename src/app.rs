use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::vec;

use crate::AppError;
use crate::cli::{self, PathListSource, PatternSource};
use crate::discovery::{DiscoveryOptions, walker};
use crate::execution::cancel::CancellationToken;
use crate::input::binary::BinaryPolicy;
use crate::input::paths as path_input;
use crate::input::patterns as pattern_input;
use crate::input::reader;
use crate::input::stdin;
use crate::output::{OutputMode, TerminalOptions, color, terminal};
use crate::search::context::ContextSelector;
use crate::search::{LineMatch, PatternMatcher, SearchLine};

#[derive(Debug)]
pub enum SearchOutcome {
    Selected,
    NoSelection,
    Errors,
}

#[derive(Debug, Clone, Copy)]
struct FileSearchOptions<'a> {
    terminal: TerminalOptions,
    before_context: usize,
    after_context: usize,
    max_count: Option<usize>,
    quiet: bool,
    output_mode: OutputMode,
    heading: bool,
    cancellation: &'a CancellationToken,
    binary_policy: BinaryPolicy,
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
        terminal: TerminalOptions {
            line_numbers: config.line_numbers,
            show_filename: config.show_filename,
            column: config.column,
            null: config.null,
            color: config.color.enabled(io::stdout().is_terminal()),
        },
        before_context: config.before_context,
        after_context: config.after_context,
        max_count: config.max_count,
        quiet: config.quiet,
        output_mode: config.output_mode,
        heading: config.heading,
        cancellation: &cancellation,
        binary_policy: config.binary_policy,
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
    } else {
        'roots: for root in roots {
            cancellation.check()?;

            let candidates = match walker::files(&root, &config.discovery) {
                Ok(candidates) => candidates,
                Err(error) => {
                    let error = AppError::Discovery(Box::new(error));

                    terminal::write_diagnostic(&mut diagnostics, &error)
                        .map_err(AppError::Diagnostic)?;
                    had_errors = true;
                    continue;
                }
            };

            for candidate in candidates {
                cancellation.check()?;

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
                    break 'roots;
                }
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

    'roots: for root in roots {
        cancellation.check()?;

        let candidates = match walker::files(root, discovery) {
            Ok(candidates) => candidates,
            Err(error) => {
                let error = AppError::Discovery(Box::new(error));

                terminal::write_diagnostic(&mut diagnostics, &error)
                    .map_err(AppError::Diagnostic)?;
                had_errors = true;
                continue;
            }
        };

        for candidate in candidates {
            cancellation.check()?;

            match candidate {
                Ok(path) => {
                    found_file = true;

                    if quiet {
                        break 'roots;
                    }

                    if let Err(source) = terminal::write_path(&mut writer, &path, options) {
                        return finish_output_error(source, had_errors, &mut diagnostics);
                    }
                }
                Err(error) => {
                    let error = AppError::Discovery(Box::new(error));

                    terminal::write_diagnostic(&mut diagnostics, &error)
                        .map_err(AppError::Diagnostic)?;
                    had_errors = true;
                }
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
    options.cancellation.check()?;

    if options.max_count == Some(0) || *remaining_results == Some(0) {
        return Ok(false);
    }

    let input = reader::open(path).map_err(|source| AppError::Open {
        path: path.to_path_buf(),
        source,
    })?;

    search_reader(
        input,
        path,
        matcher,
        options,
        remaining_results,
        headings,
        writer,
    )
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
        terminal,
        before_context,
        after_context,
        max_count,
        quiet,
        output_mode,
        heading,
        cancellation,
        binary_policy,
    } = options;

    cancellation.check()?;

    if max_count == Some(0) || *remaining_results == Some(0) {
        return Ok(false);
    }

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

    cancellation.check()?;

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
            cancellation: &cancellation,
            terminal: TerminalOptions::default(),
            before_context: 0,
            after_context: 0,
            max_count: None,
            quiet: false,
            output_mode: OutputMode::Lines,
            heading: false,
            binary_policy: BinaryPolicy::Skip,
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
                cancellation: &cancellation,
                terminal: TerminalOptions::default(),
                before_context: 0,
                after_context: 0,
                max_count: None,
                quiet: false,
                output_mode,
                heading: false,
                binary_policy: BinaryPolicy::Skip,
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
