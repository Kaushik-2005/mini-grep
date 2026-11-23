use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

use crate::execution::stats::SearchStats;
use crate::execution::{ExecutionEvent, OutputMode};
use crate::search::{MatchSpan, SearchEvent, SearchLine};

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Source<'a> {
    File { path: EncodedPath<'a> },
    Stdin,
}

impl<'a> Source<'a> {
    pub(crate) fn file(path: &'a Path) -> Self {
        Self::File {
            path: EncodedPath::from_path(path),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "encoding", content = "value", rename_all = "snake_case")]
pub(crate) enum EncodedPath<'a> {
    Utf8(&'a str),

    #[cfg(unix)]
    UnixBytes(&'a [u8]),

    #[cfg(windows)]
    WindowsUtf16(Vec<u16>),

    #[cfg(not(any(unix, windows)))]
    PlatformBytes(&'a [u8]),
}

impl<'a> EncodedPath<'a> {
    pub(crate) fn from_path(path: &'a Path) -> Self {
        if let Some(text) = path.to_str() {
            return Self::Utf8(text);
        }

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;

            Self::UnixBytes(path.as_os_str().as_bytes())
        }

        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;

            Self::WindowsUtf16(path.as_os_str().encode_wide().collect())
        }

        #[cfg(not(any(unix, windows)))]
        {
            Self::PlatformBytes(path.as_os_str().as_encoded_bytes())
        }
    }
}

#[derive(Serialize)]
struct Record<'a, 'path, T> {
    schema_version: u8,
    source: &'a Source<'path>,
    #[serde(flatten)]
    data: &'a T,
}

#[derive(Serialize)]
struct LineRecord<'a> {
    #[serde(rename = "type")]
    event_type: &'static str,
    line_number: usize,
    selected: bool,
    text: &'a str,
    spans: SpanList<'a>,
}

#[derive(Serialize)]
struct SpanRecord {
    start: usize,
    end: usize,
}

struct SpanList<'a>(&'a [MatchSpan]);

impl Serialize for SpanList<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;

        for span in self.0 {
            sequence.serialize_element(&SpanRecord {
                start: span.start(),
                end: span.end(),
            })?;
        }

        sequence.end()
    }
}

fn write_record<T: Serialize>(
    writer: &mut impl Write,
    source: &Source<'_>,
    data: &T,
) -> io::Result<()> {
    let record = Record {
        schema_version: 1,
        source,
        data,
    };

    serde_json::to_writer(&mut *writer, &record).map_err(io::Error::from)?;

    writer.write_all(b"\n")
}

fn validate_spans(line: &SearchLine) -> io::Result<()> {
    for span in &line.spans {
        if line.text.get(span.start()..span.end()).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "match span is invalid for the supplied line",
            ));
        }
    }

    Ok(())
}

pub(crate) fn write_line(
    writer: &mut impl Write,
    source: &Source<'_>,
    line: &SearchLine,
) -> io::Result<()> {
    validate_spans(line)?;

    write_record(
        writer,
        source,
        &LineRecord {
            event_type: "line",
            line_number: line.line_number,
            selected: line.selected,
            text: &line.text,
            spans: SpanList(&line.spans),
        },
    )
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CountUnit {
    SelectedLines,
    Matches,
}

#[derive(Serialize)]
struct CountRecord<'a> {
    #[serde(rename = "type")]
    event_type: &'static str,
    count: &'a str,
    unit: CountUnit,
}

pub(crate) fn write_count(
    writer: &mut impl Write,
    source: &Source<'_>,
    count: u128,
    unit: CountUnit,
) -> io::Result<()> {
    let count = count.to_string();

    write_record(
        writer,
        source,
        &CountRecord {
            event_type: "count",
            count: &count,
            unit,
        },
    )
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MarkerRecord {
    Begin,
    Path,
    GroupBreak,
}

pub(crate) fn write_begin(writer: &mut impl Write, source: &Source<'_>) -> io::Result<()> {
    write_record(writer, source, &MarkerRecord::Begin)
}

pub(crate) fn write_path(writer: &mut impl Write, source: &Source<'_>) -> io::Result<()> {
    write_record(writer, source, &MarkerRecord::Path)
}

pub(crate) fn write_group_break(writer: &mut impl Write, source: &Source<'_>) -> io::Result<()> {
    write_record(writer, source, &MarkerRecord::GroupBreak)
}

#[derive(Serialize)]
struct MatchRecord<'a> {
    #[serde(rename = "type")]
    event_type: &'static str,
    line_number: usize,
    text: &'a str,
    start: usize,
    end: usize,
}

pub(crate) fn write_only_matching(
    writer: &mut impl Write,
    source: &Source<'_>,
    line: &SearchLine,
) -> io::Result<()> {
    validate_spans(line)?;

    for span in &line.spans {
        if span.is_empty() {
            continue;
        }

        let text = line.text.get(span.start()..span.end()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "match span is invalid for the supplied line",
            )
        })?;

        write_record(
            writer,
            source,
            &MatchRecord {
                event_type: "match",
                line_number: line.line_number,
                text,
                start: span.start(),
                end: span.end(),
            },
        )?;
    }

    Ok(())
}

pub(crate) fn write_execution_event(
    writer: &mut impl Write,
    source: &Source<'_>,
    event: &ExecutionEvent,
    output_mode: OutputMode,
) -> io::Result<()> {
    match event {
        ExecutionEvent::BeginFile => write_begin(writer, source),
        ExecutionEvent::Path => write_path(writer, source),
        ExecutionEvent::Count(count) => {
            let unit = match output_mode {
                OutputMode::CountLines => CountUnit::SelectedLines,
                OutputMode::CountMatches => CountUnit::Matches,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "count event requires a count output mode",
                    ));
                }
            };

            write_count(writer, source, *count, unit)
        }
        ExecutionEvent::Search(SearchEvent::Line(line)) => write_line(writer, source, line),
        ExecutionEvent::Search(SearchEvent::GroupBreak) => write_group_break(writer, source),
        ExecutionEvent::OnlyMatching(line) => write_only_matching(writer, source, line),
    }
}

#[derive(Serialize)]
struct SummaryRecord {
    schema_version: u8,
    #[serde(rename = "type")]
    event_type: &'static str,
    inputs_started: String,
    inputs_with_selected_lines: String,
    selected_lines: String,
    matches: String,
    errors: String,
    elapsed_nanoseconds: String,
}

/// Writes a run-wide summary as one JSON Lines record.
pub(crate) fn write_summary(
    writer: &mut impl Write,
    stats: &SearchStats,
    elapsed: Duration,
) -> io::Result<()> {
    let record = SummaryRecord {
        schema_version: 1,
        event_type: "summary",
        inputs_started: stats.inputs_started.to_string(),
        inputs_with_selected_lines: stats.inputs_with_selected_lines.to_string(),
        selected_lines: stats.selected_lines.to_string(),
        matches: stats.matches.to_string(),
        errors: stats.errors.to_string(),
        elapsed_nanoseconds: elapsed.as_nanos().to_string(),
    };

    serde_json::to_writer(&mut *writer, &record).map_err(io::Error::from)?;
    writer.write_all(b"\n")
}

/// Writes one standalone JSON document followed by a newline.
pub(crate) fn write_document<T: Serialize>(
    writer: &mut impl Write,
    document: &T,
) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, document).map_err(io::Error::from)?;
    writer.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_unicode_paths_as_utf8_text() {
        let source = Source::file(Path::new("nested/café.txt"));
        let value = serde_json::to_value(source).expect("source must serialize");

        assert_eq!(
            value,
            json!({
                "kind": "file",
                "path": {
                    "encoding": "utf8",
                    "value": "nested/café.txt"
                }
            }),
        );
    }

    #[test]
    fn escapes_quotes_and_line_endings_without_losing_path_text() {
        let path = Path::new("quoted\"name\nnext\r.txt");
        let source = Source::file(path);
        let encoded = serde_json::to_string(&source).expect("source must serialize");

        assert!(!encoded.contains('\n'));
        assert!(!encoded.contains('\r'));

        let value: serde_json::Value =
            serde_json::from_str(&encoded).expect("encoded source must be valid JSON");

        assert_eq!(value["path"]["value"], json!("quoted\"name\nnext\r.txt"),);
    }

    #[test]
    fn serializes_stdin_without_a_file_path() {
        let value = serde_json::to_value(Source::Stdin).expect("stdin must serialize");

        assert_eq!(value, json!({ "kind": "stdin" }));
    }

    #[test]
    fn does_not_confuse_a_file_with_the_stdin_display_label() {
        let source = Source::file(Path::new("(standard input)"));
        let value = serde_json::to_value(source).expect("source must serialize");

        assert_eq!(
            value,
            json!({
                "kind": "file",
                "path": {
                    "encoding": "utf8",
                    "value": "(standard input)"
                }
            }),
        );
    }

    #[cfg(unix)]
    #[test]
    fn preserves_non_utf8_unix_path_bytes() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        use std::path::PathBuf;

        let bytes = vec![b'f', 0xff, b'\n'];
        let path = PathBuf::from(OsString::from_vec(bytes.clone()));
        let value = serde_json::to_value(Source::file(&path)).expect("native path must serialize");

        assert_eq!(
            value,
            json!({
                "kind": "file",
                "path": {
                    "encoding": "unix_bytes",
                    "value": bytes
                }
            }),
        );
    }

    #[cfg(windows)]
    #[test]
    fn preserves_unpaired_windows_utf16_units() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use std::path::PathBuf;

        let units = vec![0xd800_u16, 0x002e, 0x0072, 0x0073];
        let path = PathBuf::from(OsString::from_wide(&units));
        let value = serde_json::to_value(Source::file(&path)).expect("native path must serialize");

        assert_eq!(
            value,
            json!({
                "kind": "file",
                "path": {
                    "encoding": "windows_utf16",
                    "value": units
                }
            }),
        );
    }

    #[test]
    fn writes_selected_lines_with_utf8_byte_spans() -> io::Result<()> {
        let text = String::from("é Rust");
        let span = MatchSpan::new(&text, 3, 7).expect("span must cover Rust");
        let line = SearchLine {
            line_number: 2,
            text,
            spans: vec![span],
            selected: true,
        };
        let source = Source::file(Path::new("demo.txt"));
        let mut output = Vec::new();

        write_line(&mut output, &source, &line)?;

        assert_eq!(output.last(), Some(&b'\n'));
        assert_eq!(output.iter().filter(|&&byte| byte == b'\n').count(), 1,);

        let value: serde_json::Value = serde_json::from_slice(&output).expect("record must parse");

        assert_eq!(
            value,
            json!({
                "schema_version": 1,
                "type": "line",
                "source": {
                    "kind": "file",
                    "path": {
                        "encoding": "utf8",
                        "value": "demo.txt"
                    }
                },
                "line_number": 2,
                "selected": true,
                "text": "é Rust",
                "spans": [{ "start": 3, "end": 7 }]
            }),
        );

        Ok(())
    }

    #[test]
    fn writes_independent_records_and_escapes_control_characters() -> io::Result<()> {
        let context = SearchLine {
            line_number: 1,
            text: String::from("quote\"\t\n\0"),
            spans: Vec::new(),
            selected: false,
        };
        let empty = SearchLine {
            line_number: 2,
            text: String::new(),
            spans: vec![MatchSpan::new("", 0, 0).expect("empty span must be valid")],
            selected: true,
        };
        let mut output = Vec::new();

        write_line(&mut output, &Source::Stdin, &context)?;
        write_line(&mut output, &Source::Stdin, &empty)?;

        let text = String::from_utf8(output).expect("JSON must be UTF-8");
        let records: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).expect("each line must parse independently"))
            .collect();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["source"], json!({ "kind": "stdin" }));
        assert_eq!(records[0]["selected"], json!(false));
        assert_eq!(records[0]["text"], json!("quote\"\t\n\0"));
        assert_eq!(records[0]["spans"], json!([]));
        assert_eq!(records[1]["text"], json!(""));
        assert_eq!(records[1]["selected"], json!(true));
        assert_eq!(records[1]["spans"], json!([{ "start": 0, "end": 0 }]),);

        Ok(())
    }

    #[test]
    fn rejects_invalid_spans_before_writing_a_record() {
        let span = MatchSpan::new("Rust", 0, 4).expect("span must be valid for its original text");
        let line = SearchLine {
            line_number: 1,
            text: String::from("é"),
            spans: vec![span],
            selected: true,
        };
        let mut output = Vec::new();

        let error = write_line(&mut output, &Source::Stdin, &line)
            .expect_err("span must be rejected for this line");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(output.is_empty());
    }

    struct FailingWriter {
        kind: io::ErrorKind,
    }

    impl Write for FailingWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(self.kind, "simulated write failure"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn preserves_output_error_kinds() {
        let line = SearchLine {
            line_number: 1,
            text: String::from("Rust"),
            spans: Vec::new(),
            selected: true,
        };

        for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::Other] {
            let mut writer = FailingWriter { kind };

            let error = write_line(&mut writer, &Source::Stdin, &line)
                .expect_err("writer failure must propagate");

            assert_eq!(error.kind(), kind);
        }
    }

    #[test]
    fn writes_selected_line_counts_without_integer_precision_loss() -> io::Result<()> {
        for (count, expected) in [
            (0_u128, "0"),
            (42_u128, "42"),
            (u128::MAX, "340282366920938463463374607431768211455"),
        ] {
            let mut output = Vec::new();

            write_count(&mut output, &Source::Stdin, count, CountUnit::SelectedLines)?;

            let value: serde_json::Value =
                serde_json::from_slice(&output).expect("count record must parse");

            assert_eq!(
                value,
                json!({
                    "schema_version": 1,
                    "source": { "kind": "stdin" },
                    "type": "count",
                    "count": expected,
                    "unit": "selected_lines"
                }),
            );
        }

        Ok(())
    }

    #[test]
    fn distinguishes_match_counts_from_selected_line_counts() -> io::Result<()> {
        let source = Source::file(Path::new("demo.txt"));
        let mut output = Vec::new();

        write_count(&mut output, &source, 2, CountUnit::Matches)?;

        let value: serde_json::Value =
            serde_json::from_slice(&output).expect("count record must parse");

        assert_eq!(value["type"], json!("count"));
        assert_eq!(value["count"], json!("2"));
        assert_eq!(value["unit"], json!("matches"));

        Ok(())
    }

    #[test]
    fn writes_begin_path_and_group_break_as_independent_records() -> io::Result<()> {
        let source = Source::file(Path::new("demo.txt"));
        let mut output = Vec::new();

        write_begin(&mut output, &source)?;
        write_path(&mut output, &source)?;
        write_group_break(&mut output, &source)?;

        let text = String::from_utf8(output).expect("JSON must be UTF-8");
        let records: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).expect("each record must parse independently"))
            .collect();

        assert_eq!(records.len(), 3);
        assert!(text.ends_with('\n'));

        for (record, expected_type) in records.iter().zip(["begin", "path", "group_break"]) {
            assert_eq!(
                record,
                &json!({
                    "schema_version": 1,
                    "source": {
                        "kind": "file",
                        "path": {
                            "encoding": "utf8",
                            "value": "demo.txt"
                        }
                    },
                    "type": expected_type
                }),
            );
        }

        Ok(())
    }

    #[test]
    fn match_only_preserves_original_text_offsets_and_skips_empty_spans() -> io::Result<()> {
        let text = String::from("é Rust RUST");
        let spans = vec![
            MatchSpan::new(&text, 0, 0).expect("empty span must be valid"),
            MatchSpan::new(&text, 3, 7).expect("first match must be valid"),
            MatchSpan::new(&text, 8, 12).expect("second match must be valid"),
            MatchSpan::new(&text, 12, 12).expect("final empty span must be valid"),
        ];
        let line = SearchLine {
            line_number: 5,
            text,
            spans,
            selected: true,
        };
        let mut output = Vec::new();

        write_only_matching(&mut output, &Source::Stdin, &line)?;

        let text = String::from_utf8(output).expect("JSON must be UTF-8");
        let records: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).expect("each match record must parse"))
            .collect();

        assert_eq!(records.len(), 2);

        for (record, (matched, start, end)) in records.iter().zip([("Rust", 3, 7), ("RUST", 8, 12)])
        {
            assert_eq!(
                record,
                &json!({
                    "schema_version": 1,
                    "source": { "kind": "stdin" },
                    "type": "match",
                    "line_number": 5,
                    "text": matched,
                    "start": start,
                    "end": end
                }),
            );
        }

        Ok(())
    }

    #[test]
    fn invalid_later_span_prevents_all_match_only_output() {
        let line = SearchLine {
            line_number: 1,
            text: String::from("Rust"),
            spans: vec![
                MatchSpan::new("Rust", 0, 4).expect("first span must be valid"),
                MatchSpan::new("Rust Rust", 5, 9).expect("second span is valid for longer text"),
            ],
            selected: true,
        };
        let mut output = Vec::new();

        let error = write_only_matching(&mut output, &Source::Stdin, &line)
            .expect_err("invalid later span must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(output.is_empty());
    }

    #[test]
    fn dispatches_non_count_execution_events() -> io::Result<()> {
        let context = SearchLine {
            line_number: 2,
            text: String::from("before"),
            spans: Vec::new(),
            selected: false,
        };
        let matched = SearchLine {
            line_number: 3,
            text: String::from("Rust"),
            spans: vec![MatchSpan::new("Rust", 0, 4).expect("match span must be valid")],
            selected: true,
        };

        let cases = [
            (ExecutionEvent::BeginFile, OutputMode::Lines, "begin"),
            (ExecutionEvent::Path, OutputMode::FilesWithMatches, "path"),
            (
                ExecutionEvent::Search(SearchEvent::Line(context)),
                OutputMode::Lines,
                "line",
            ),
            (
                ExecutionEvent::Search(SearchEvent::GroupBreak),
                OutputMode::Lines,
                "group_break",
            ),
            (
                ExecutionEvent::OnlyMatching(matched),
                OutputMode::OnlyMatching,
                "match",
            ),
        ];

        for (event, mode, expected_type) in cases {
            let mut output = Vec::new();

            write_execution_event(&mut output, &Source::Stdin, &event, mode)?;

            let value: serde_json::Value =
                serde_json::from_slice(&output).expect("dispatched record must parse");

            assert_eq!(value["schema_version"], json!(1));
            assert_eq!(value["source"], json!({ "kind": "stdin" }));
            assert_eq!(value["type"], json!(expected_type));
        }

        Ok(())
    }

    #[test]
    fn dispatches_count_events_with_the_correct_unit() -> io::Result<()> {
        for (mode, expected_unit) in [
            (OutputMode::CountLines, "selected_lines"),
            (OutputMode::CountMatches, "matches"),
        ] {
            let mut output = Vec::new();

            write_execution_event(&mut output, &Source::Stdin, &ExecutionEvent::Count(7), mode)?;

            let value: serde_json::Value =
                serde_json::from_slice(&output).expect("count record must parse");

            assert_eq!(value["type"], json!("count"));
            assert_eq!(value["count"], json!("7"));
            assert_eq!(value["unit"], json!(expected_unit));
        }

        Ok(())
    }

    #[test]
    fn rejects_count_events_outside_count_modes() {
        for mode in [
            OutputMode::Lines,
            OutputMode::FilesWithMatches,
            OutputMode::FilesWithoutMatch,
            OutputMode::OnlyMatching,
        ] {
            let mut output = Vec::new();

            let error =
                write_execution_event(&mut output, &Source::Stdin, &ExecutionEvent::Count(7), mode)
                    .expect_err("invalid event mode must fail");

            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert!(output.is_empty());
        }
    }

    #[test]
    fn summary_preserves_large_counters_and_exact_elapsed_nanoseconds() {
        let stats = SearchStats {
            inputs_started: 3,
            inputs_with_selected_lines: 2,
            selected_lines: 4,
            matches: u128::MAX,
            errors: 1,
        };

        let mut output = Vec::new();
        write_summary(&mut output, &stats, Duration::from_millis(1250))
            .expect("in-memory summary output must succeed");

        assert_eq!(output.iter().filter(|&&byte| byte == b'\n').count(), 1);
        assert!(output.ends_with(b"\n"));

        let record: serde_json::Value =
            serde_json::from_slice(&output).expect("summary must be valid JSON");

        assert_eq!(
            record,
            serde_json::json!({
                "schema_version": 1,
                "type": "summary",
                "inputs_started": "3",
                "inputs_with_selected_lines": "2",
                "selected_lines": "4",
                "matches": u128::MAX.to_string(),
                "errors": "1",
                "elapsed_nanoseconds": "1250000000"
            })
        );
    }

    #[test]
    fn summary_preserves_underlying_output_error_kind() {
        struct SummaryFailingWriter;

        impl Write for SummaryFailingWriter {
            fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "summary pipe closed",
                ))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let error = write_summary(
            &mut SummaryFailingWriter,
            &SearchStats::default(),
            Duration::ZERO,
        )
        .expect_err("summary write failure must propagate");

        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }
}
