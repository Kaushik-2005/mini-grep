use std::io::{self, Write};
use std::path::Path;

use super::{TerminalOptions, color};
use crate::AppError;
use crate::execution::ExecutionEvent;
use crate::search::{LineMatch, MatchSpan, SearchEvent};

pub(crate) fn write_match(
    writer: &mut impl Write,
    path: &Path,
    matched_line: &LineMatch<'_>,
    options: TerminalOptions,
) -> io::Result<()> {
    let column = matched_line
        .spans
        .first()
        .map_or(1, |span| span.start() + 1);

    write_line(
        writer,
        path,
        matched_line.line_number,
        column,
        (matched_line.text, &matched_line.spans),
        ':',
        options,
    )
}

pub(crate) fn write_diagnostic(writer: &mut impl Write, error: &AppError) -> io::Result<()> {
    writeln!(writer, "mini-grep: {error}")
}

pub(crate) fn write_event(
    writer: &mut impl Write,
    path: &Path,
    event: &SearchEvent,
    options: TerminalOptions,
) -> io::Result<()> {
    match event {
        SearchEvent::GroupBreak => writeln!(writer, "--"),
        SearchEvent::Line(line) => {
            let column = if line.selected {
                line.spans.first().map_or(1, |span| span.start() + 1)
            } else {
                1
            };

            write_line(
                writer,
                path,
                line.line_number,
                column,
                (&line.text, if line.selected { &line.spans } else { &[] }),
                if line.selected { ':' } else { '-' },
                options,
            )
        }
    }
}

pub(crate) fn write_path(
    writer: &mut impl Write,
    path: &Path,
    options: TerminalOptions,
) -> io::Result<()> {
    write_filename(writer, path, options.null, b'\n')
}

pub(crate) fn write_count(
    writer: &mut impl Write,
    path: &Path,
    count: u128,
    options: TerminalOptions,
) -> io::Result<()> {
    if options.show_filename {
        write_filename(writer, path, options.null, b':')?;
    }

    writeln!(writer, "{count}")
}

pub(crate) fn write_only_matching(
    writer: &mut impl Write,
    path: &Path,
    line: &LineMatch<'_>,
    options: TerminalOptions,
) -> io::Result<()> {
    for span in &line.spans {
        let matched = line.text.get(span.start()..span.end()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "match span is invalid for the supplied line",
            )
        })?;

        if matched.is_empty() {
            continue;
        }

        let highlighted_span = MatchSpan::new(matched, 0, matched.len()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "matched text must form a valid complete span",
            )
        })?;
        let highlighted_spans = [highlighted_span];

        write_line(
            writer,
            path,
            line.line_number,
            span.start() + 1,
            (matched, &highlighted_spans),
            ':',
            options,
        )?;
    }

    Ok(())
}

fn write_line(
    writer: &mut impl Write,
    path: &Path,
    line_number: usize,
    column: usize,
    content: (&str, &[MatchSpan]),
    separator: char,
    options: TerminalOptions,
) -> io::Result<()> {
    let (text, spans) = content;

    if options.show_filename {
        write_filename(writer, path, options.null, separator as u8)?;
    }

    if options.line_numbers || options.column {
        write!(writer, "{line_number}{separator}")?;
    }

    if options.column {
        write!(writer, "{column}{separator}")?;
    }

    if options.color {
        color::write_highlighted(writer, text, spans)?;
    } else {
        writer.write_all(text.as_bytes())?;
    }

    writeln!(writer)
}

#[derive(Debug, Default)]
pub(crate) struct HeadingState {
    any_file_started: bool,
    current_file_started: bool,
}

impl HeadingState {
    pub(crate) fn begin_file(&mut self) {
        self.current_file_started = false;
    }

    pub(crate) fn before_record(&mut self, writer: &mut impl Write, path: &Path) -> io::Result<()> {
        if self.current_file_started {
            return Ok(());
        }

        if self.any_file_started {
            writeln!(writer)?;
        }

        writeln!(writer, "{}", path.display())?;

        self.current_file_started = true;
        self.any_file_started = true;

        Ok(())
    }
}

fn write_filename(
    writer: &mut impl Write,
    path: &Path,
    null: bool,
    delimiter: u8,
) -> io::Result<()> {
    if null {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;

            writer.write_all(path.as_os_str().as_bytes())?;
        }

        #[cfg(not(unix))]
        {
            let text = path.to_str().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "cannot encode filename losslessly as UTF-8",
                )
            })?;

            writer.write_all(text.as_bytes())?;
        }

        writer.write_all(b"\0")
    } else {
        write!(writer, "{}", path.display())?;
        writer.write_all(&[delimiter])
    }
}

pub(crate) fn write_execution_event(
    writer: &mut impl Write,
    path: &Path,
    event: ExecutionEvent,
    options: TerminalOptions,
    heading: bool,
    headings: &mut HeadingState,
) -> io::Result<()> {
    let visible_line = match &event {
        ExecutionEvent::Search(SearchEvent::Line(_)) => true,
        ExecutionEvent::OnlyMatching(line) => line.spans.iter().any(|span| !span.is_empty()),
        _ => false,
    };

    if heading && visible_line {
        headings.before_record(writer, path)?;
    }

    let line_options = TerminalOptions {
        show_filename: options.show_filename && !heading,
        ..options
    };

    match event {
        ExecutionEvent::BeginFile => {
            headings.begin_file();
            Ok(())
        }
        ExecutionEvent::Path => write_path(writer, path, options),
        ExecutionEvent::Count(count) => write_count(writer, path, count, options),
        ExecutionEvent::Search(SearchEvent::Line(line)) if line.selected => {
            let matched = LineMatch {
                line_number: line.line_number,
                text: &line.text,
                spans: line.spans,
            };

            write_match(writer, path, &matched, line_options)
        }
        ExecutionEvent::Search(event) => write_event(writer, path, &event, line_options),
        ExecutionEvent::OnlyMatching(line) => {
            let matched = LineMatch {
                line_number: line.line_number,
                text: &line.text,
                spans: line.spans,
            };

            write_only_matching(writer, path, &matched, line_options)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format_options(line_numbers: bool) -> TerminalOptions {
        TerminalOptions {
            line_numbers,
            ..TerminalOptions::default()
        }
    }

    #[test]
    fn writes_path_line_number_and_text_when_enabled() -> io::Result<()> {
        let mut output = Vec::new();
        let matched_line = LineMatch {
            line_number: 7,
            text: "नमस्ते Rust",
            spans: Vec::new(),
        };

        write_match(
            &mut output,
            Path::new("sample file.txt"),
            &matched_line,
            format_options(true),
        )?;

        assert_eq!(output, "sample file.txt:7:नमस्ते Rust\n".as_bytes());

        Ok(())
    }

    #[test]
    fn omits_line_numbers_when_disabled() -> io::Result<()> {
        let mut output = Vec::new();
        let matched_line = LineMatch {
            line_number: 7,
            text: "Rust",
            spans: Vec::new(),
        };

        write_match(
            &mut output,
            Path::new("sample.txt"),
            &matched_line,
            format_options(false),
        )?;

        assert_eq!(output, b"sample.txt:Rust\n");

        Ok(())
    }

    #[test]
    fn writes_an_empty_matching_line() -> io::Result<()> {
        let mut output = Vec::new();
        let matched_line = LineMatch {
            line_number: 2,
            text: "",
            spans: Vec::new(),
        };

        write_match(
            &mut output,
            Path::new("sample.txt"),
            &matched_line,
            format_options(false),
        )?;

        assert_eq!(output, b"sample.txt:\n");

        Ok(())
    }

    #[test]
    fn propagates_write_errors_in_both_output_modes() {
        for line_numbers in [false, true] {
            let mut output: &mut [u8] = &mut [];
            let matched_line = LineMatch {
                line_number: 1,
                text: "Rust",
                spans: Vec::new(),
            };

            let error = write_match(
                &mut output,
                Path::new("sample.txt"),
                &matched_line,
                format_options(line_numbers),
            )
            .expect_err("an empty fixed-size buffer cannot accept output");

            assert_eq!(error.kind(), io::ErrorKind::WriteZero);
        }
    }

    #[test]
    fn writes_a_contextual_error_diagnostic() -> io::Result<()> {
        let mut output = Vec::new();
        let error = AppError::Read {
            path: Path::new("sample.txt").to_path_buf(),
            source: io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8"),
        };

        write_diagnostic(&mut output, &error)?;

        assert_eq!(
            output,
            b"mini-grep: cannot read 'sample.txt': invalid UTF-8\n"
        );

        Ok(())
    }

    #[test]
    fn propagates_diagnostic_write_errors() {
        let mut output: &mut [u8] = &mut [];
        let error = AppError::Usage("invalid arguments");

        let failure = write_diagnostic(&mut output, &error)
            .expect_err("an empty fixed-size buffer cannot accept output");

        assert_eq!(failure.kind(), io::ErrorKind::WriteZero);
    }

    #[test]
    fn renders_selected_and_context_lines_in_both_number_modes() -> io::Result<()> {
        use crate::search::SearchLine;

        for (selected, numbered, expected) in [
            (true, true, "sample.txt:7:é Rust\n"),
            (true, false, "sample.txt:é Rust\n"),
            (false, true, "sample.txt-7-é Rust\n"),
            (false, false, "sample.txt-é Rust\n"),
        ] {
            let event = SearchEvent::Line(SearchLine {
                line_number: 7,
                text: String::from("é Rust"),
                spans: Vec::new(),
                selected,
            });
            let mut output = Vec::new();

            write_event(
                &mut output,
                Path::new("sample.txt"),
                &event,
                format_options(numbered),
            )?;

            assert_eq!(output, expected.as_bytes());
        }

        Ok(())
    }

    #[test]
    fn renders_empty_context_lines_and_group_breaks() -> io::Result<()> {
        use crate::search::SearchLine;

        for (numbered, expected) in [(true, "sample.txt-2-\n--\n"), (false, "sample.txt-\n--\n")] {
            let event = SearchEvent::Line(SearchLine {
                line_number: 2,
                text: String::new(),
                spans: Vec::new(),
                selected: false,
            });
            let mut output = Vec::new();

            write_event(
                &mut output,
                Path::new("sample.txt"),
                &event,
                format_options(numbered),
            )?;
            write_event(
                &mut output,
                Path::new("sample.txt"),
                &SearchEvent::GroupBreak,
                format_options(numbered),
            )?;

            assert_eq!(output, expected.as_bytes());
        }

        Ok(())
    }

    #[test]
    fn event_write_errors_are_propagated() {
        use crate::search::SearchLine;

        let events = [
            SearchEvent::GroupBreak,
            SearchEvent::Line(SearchLine {
                line_number: 1,
                text: String::from("selected"),
                spans: Vec::new(),
                selected: true,
            }),
            SearchEvent::Line(SearchLine {
                line_number: 2,
                text: String::from("context"),
                spans: Vec::new(),
                selected: false,
            }),
        ];

        for event in &events {
            for numbered in [false, true] {
                let mut output: &mut [u8] = &mut [];

                let error = write_event(
                    &mut output,
                    Path::new("sample.txt"),
                    event,
                    format_options(numbered),
                )
                .expect_err("an empty fixed-size buffer cannot accept output");

                assert_eq!(error.kind(), io::ErrorKind::WriteZero);
            }
        }
    }

    #[test]
    fn writes_file_paths_without_line_details() -> io::Result<()> {
        let mut output = Vec::new();

        write_path(
            &mut output,
            Path::new("sample file.txt"),
            TerminalOptions::default(),
        )?;
        write_path(
            &mut output,
            Path::new("café.txt"),
            TerminalOptions::default(),
        )?;

        assert_eq!(output, "sample file.txt\ncafé.txt\n".as_bytes());

        Ok(())
    }

    #[test]
    fn path_write_errors_are_propagated() {
        let mut output: &mut [u8] = &mut [];

        let error = write_path(
            &mut output,
            Path::new("sample.txt"),
            TerminalOptions::default(),
        )
        .expect_err("an empty fixed-size buffer cannot accept output");

        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    }

    #[test]
    fn writes_zero_and_large_counts() -> io::Result<()> {
        let mut output = Vec::new();
        let large = u128::from(u64::MAX) + 1;

        write_count(
            &mut output,
            Path::new("sample file.txt"),
            0,
            TerminalOptions::default(),
        )?;
        write_count(
            &mut output,
            Path::new("café.txt"),
            large,
            TerminalOptions::default(),
        )?;

        assert_eq!(
            output,
            "sample file.txt:0\ncafé.txt:18446744073709551616\n".as_bytes()
        );

        Ok(())
    }

    #[test]
    fn count_write_errors_are_propagated() {
        let mut output: &mut [u8] = &mut [];

        let error = write_count(
            &mut output,
            Path::new("sample.txt"),
            3,
            TerminalOptions::default(),
        )
        .expect_err("an empty fixed-size buffer cannot accept output");

        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    }

    #[test]
    fn match_only_output_uses_original_byte_ranges_and_skips_empty_spans() -> io::Result<()> {
        use crate::search::MatchSpan;

        let text = "é rUsT Rust";
        let line = LineMatch {
            line_number: 7,
            text,
            spans: vec![
                MatchSpan::new(text, 0, 0).expect("valid empty span"),
                MatchSpan::new(text, 3, 7).expect("valid first match"),
                MatchSpan::new(text, 8, 12).expect("valid second match"),
                MatchSpan::new(text, 12, 12).expect("valid empty end span"),
            ],
        };

        for (numbered, expected) in [
            (false, "sample.txt:rUsT\nsample.txt:Rust\n"),
            (true, "sample.txt:7:rUsT\nsample.txt:7:Rust\n"),
        ] {
            let mut output = Vec::new();

            write_only_matching(
                &mut output,
                Path::new("sample.txt"),
                &line,
                format_options(numbered),
            )?;

            assert_eq!(output, expected.as_bytes());
        }

        Ok(())
    }

    #[test]
    fn match_only_output_rejects_spans_from_incompatible_text() {
        use crate::search::MatchSpan;

        let line = LineMatch {
            line_number: 1,
            text: "é",
            spans: vec![MatchSpan::new("Rust", 0, 4).expect("valid range in the other text")],
        };
        let mut output = Vec::new();

        let error = write_only_matching(
            &mut output,
            Path::new("sample.txt"),
            &line,
            format_options(false),
        )
        .expect_err("the span exceeds the supplied line");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(output.is_empty());
    }

    #[test]
    fn match_only_output_propagates_write_errors() {
        use crate::search::MatchSpan;

        let line = LineMatch {
            line_number: 1,
            text: "Rust",
            spans: vec![MatchSpan::new("Rust", 0, 4).expect("valid match range")],
        };

        for numbered in [false, true] {
            let mut output: &mut [u8] = &mut [];

            let error = write_only_matching(
                &mut output,
                Path::new("sample.txt"),
                &line,
                format_options(numbered),
            )
            .expect_err("an empty fixed-size buffer cannot accept output");

            assert_eq!(error.kind(), io::ErrorKind::WriteZero);
        }
    }

    #[test]
    fn line_prefixes_respect_filename_and_number_visibility() -> io::Result<()> {
        let cases = [
            (true, true, ':', "sample.txt:7:é Rust\n"),
            (true, false, ':', "sample.txt:é Rust\n"),
            (false, true, ':', "7:é Rust\n"),
            (false, false, ':', "é Rust\n"),
            (true, true, '-', "sample.txt-7-é Rust\n"),
            (true, false, '-', "sample.txt-é Rust\n"),
            (false, true, '-', "7-é Rust\n"),
            (false, false, '-', "é Rust\n"),
        ];

        for (show_filename, line_numbers, separator, expected) in cases {
            let options = TerminalOptions {
                line_numbers,
                show_filename,
                column: false,
                null: false,
                color: false,
            };
            let mut output = Vec::new();

            write_line(
                &mut output,
                Path::new("sample.txt"),
                7,
                1,
                ("é Rust", &[]),
                separator,
                options,
            )?;

            assert_eq!(output, expected.as_bytes());
        }

        Ok(())
    }

    #[test]
    fn heading_state_skips_empty_files_and_repeats_no_heading() -> io::Result<()> {
        let mut state = HeadingState::default();
        let mut output = Vec::new();

        state.begin_file();
        state.before_record(&mut output, Path::new("first.txt"))?;
        state.before_record(&mut output, Path::new("first.txt"))?;

        // This file emits no records.
        state.begin_file();

        state.begin_file();
        state.before_record(&mut output, Path::new("second.txt"))?;

        assert_eq!(output, b"first.txt\n\nsecond.txt\n");

        Ok(())
    }

    #[test]
    fn heading_errors_propagate_for_initial_and_later_groups() {
        for previous_group in [false, true] {
            let mut state = HeadingState::default();

            if previous_group {
                let mut previous_output = Vec::new();
                state.begin_file();
                state
                    .before_record(&mut previous_output, Path::new("previous.txt"))
                    .expect("an in-memory buffer must accept the heading");
            }

            state.begin_file();
            let mut output: &mut [u8] = &mut [];

            let error = state
                .before_record(&mut output, Path::new("current.txt"))
                .expect_err("an empty fixed-size buffer cannot accept output");

            assert_eq!(error.kind(), io::ErrorKind::WriteZero);
            assert!(!state.current_file_started);
            assert_eq!(state.any_file_started, previous_group);
        }
    }

    #[test]
    fn filename_writer_uses_exact_requested_delimiters() -> io::Result<()> {
        for (null, delimiter, expected) in [
            (false, b':', "café file.txt:"),
            (false, b'-', "café file.txt-"),
            (false, b'\n', "café file.txt\n"),
            (true, b':', "café file.txt\0"),
            (true, b'-', "café file.txt\0"),
            (true, b'\n', "café file.txt\0"),
        ] {
            let mut output = Vec::new();

            write_filename(&mut output, Path::new("café file.txt"), null, delimiter)?;

            assert_eq!(output, expected.as_bytes());
        }

        Ok(())
    }

    #[test]
    fn filename_writer_propagates_write_errors() {
        for null in [false, true] {
            let mut output: &mut [u8] = &mut [];

            let error = write_filename(&mut output, Path::new("sample.txt"), null, b':')
                .expect_err("an empty fixed-size buffer cannot accept output");

            assert_eq!(error.kind(), io::ErrorKind::WriteZero);
        }
    }

    #[cfg(unix)]
    #[test]
    fn null_filenames_preserve_native_bytes_and_embedded_newlines() -> io::Result<()> {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path = Path::new(OsStr::from_bytes(b"sample-\xff\n.txt"));
        let mut output = Vec::new();

        write_filename(&mut output, path, true, b'\n')?;

        assert_eq!(output, b"sample-\xff\n.txt\0");

        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn null_filenames_reject_unpaired_utf16_surrogates() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let name = OsString::from_wide(&[0x0066, 0xD800]);
        let path = Path::new(&name);
        let mut output = Vec::new();

        let error = write_filename(&mut output, path, true, b'\n')
            .expect_err("an unpaired surrogate cannot become valid UTF-8");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(output.is_empty());
    }

    #[test]
    fn highlights_match_text_without_coloring_prefixes() -> io::Result<()> {
        let text = "é Rust!";
        let matched_line = LineMatch {
            line_number: 7,
            text,
            spans: vec![MatchSpan::new(text, 3, 7).expect("span must cover Rust")],
        };
        let options = TerminalOptions {
            line_numbers: true,
            column: true,
            color: true,
            ..TerminalOptions::default()
        };
        let mut output = Vec::new();

        write_match(&mut output, Path::new("sample.txt"), &matched_line, options)?;

        assert_eq!(
            output,
            "sample.txt:7:4:é \x1b[1;31mRust\x1b[0m!\n".as_bytes(),
        );
        Ok(())
    }
}
