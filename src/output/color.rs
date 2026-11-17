use std::io::{self, IsTerminal, StdoutLock, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorPolicy {
    Auto,
    Always,
    Never,
}

impl ColorPolicy {
    pub(crate) fn enabled(self, stdout_is_terminal: bool) -> bool {
        match self {
            Self::Auto => stdout_is_terminal,
            Self::Always => true,
            Self::Never => false,
        }
    }
}

pub(crate) enum StdoutWriter {
    Plain(StdoutLock<'static>),
    Styled(anstream::AutoStream<StdoutLock<'static>>),
}

impl Write for StdoutWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(writer) => writer.write(bytes),
            Self::Styled(writer) => writer.write(bytes),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(writer) => writer.flush(),
            Self::Styled(writer) => writer.flush(),
        }
    }
}

pub(crate) fn stdout(policy: ColorPolicy) -> StdoutWriter {
    let stdout = io::stdout();
    let enabled = policy.enabled(stdout.is_terminal());
    let writer = stdout.lock();

    if enabled {
        StdoutWriter::Styled(anstream::AutoStream::new(
            writer,
            anstream::ColorChoice::Always,
        ))
    } else {
        StdoutWriter::Plain(writer)
    }
}

pub(crate) fn write_highlighted(
    writer: &mut impl Write,
    text: &str,
    spans: &[crate::search::MatchSpan],
) -> io::Result<()> {
    let slice = |start, end| {
        text.get(start..end).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "match span is invalid for the supplied line",
            )
        })
    };

    let mut previous_start = 0;

    for span in spans {
        slice(span.start(), span.end())?;

        if span.start() < previous_start {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "match spans must be sorted by start offset",
            ));
        }

        previous_start = span.start();
    }

    let mut cursor = 0;

    {
        let mut write_region = |start, end| -> io::Result<()> {
            writer.write_all(slice(cursor, start)?.as_bytes())?;
            writer.write_all(b"\x1b[1;31m")?;
            writer.write_all(slice(start, end)?.as_bytes())?;
            writer.write_all(b"\x1b[0m")?;
            cursor = end;
            Ok(())
        };

        let mut pending: Option<(usize, usize)> = None;

        for span in spans.iter().filter(|span| !span.is_empty()) {
            pending = match pending {
                Some((start, end)) if span.start() <= end => Some((start, end.max(span.end()))),
                Some((start, end)) => {
                    write_region(start, end)?;
                    Some((span.start(), span.end()))
                }
                None => Some((span.start(), span.end())),
            };
        }

        if let Some((start, end)) = pending {
            write_region(start, end)?;
        }
    }

    writer.write_all(slice(cursor, text.len())?.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_color_from_policy_and_stdout_terminal_status() {
        let cases = [
            (ColorPolicy::Auto, false, false),
            (ColorPolicy::Auto, true, true),
            (ColorPolicy::Always, false, true),
            (ColorPolicy::Always, true, true),
            (ColorPolicy::Never, false, false),
            (ColorPolicy::Never, true, false),
        ];

        for (policy, is_terminal, expected) in cases {
            assert_eq!(
                policy.enabled(is_terminal),
                expected,
                "policy: {policy:?}; terminal: {is_terminal}",
            );
        }
    }

    #[test]
    fn highlights_unicode_using_byte_spans() -> io::Result<()> {
        let text = "é Rust!";
        let spans = [crate::search::MatchSpan::new(text, 3, 7).expect("span must cover Rust")];
        let mut output = Vec::new();

        write_highlighted(&mut output, text, &spans)?;

        assert_eq!(output, "é \x1b[1;31mRust\x1b[0m!".as_bytes());
        Ok(())
    }

    #[test]
    fn merges_overlapping_and_adjacent_spans() -> io::Result<()> {
        let text = "abcdef!";
        let spans = [
            crate::search::MatchSpan::new(text, 0, 3).expect("valid span"),
            crate::search::MatchSpan::new(text, 2, 4).expect("valid span"),
            crate::search::MatchSpan::new(text, 4, 6).expect("valid span"),
        ];
        let mut output = Vec::new();

        write_highlighted(&mut output, text, &spans)?;

        assert_eq!(output, b"\x1b[1;31mabcdef\x1b[0m!");
        Ok(())
    }

    #[test]
    fn empty_and_zero_width_spans_leave_text_plain() -> io::Result<()> {
        let text = "Rust";
        let zero_width = [
            crate::search::MatchSpan::new(text, 0, 0).expect("valid boundary"),
            crate::search::MatchSpan::new(text, 4, 4).expect("valid boundary"),
        ];

        for spans in [&[][..], &zero_width[..]] {
            let mut output = Vec::new();

            write_highlighted(&mut output, text, spans)?;

            assert_eq!(output, text.as_bytes());
        }

        Ok(())
    }

    #[test]
    fn rejects_invalid_or_unsorted_spans_before_writing() {
        let cases = [
            vec![crate::search::MatchSpan::new("Rust", 0, 4).expect("valid for the original text")],
            vec![
                crate::search::MatchSpan::new("Rust", 2, 3).expect("valid span"),
                crate::search::MatchSpan::new("Rust", 0, 1).expect("valid span"),
            ],
            vec![
                crate::search::MatchSpan::new("ab", 1, 2)
                    .expect("valid for ASCII, invalid inside é"),
            ],
        ];

        for (text, spans) in ["Go", "Rust", "é"].into_iter().zip(cases) {
            let mut output = Vec::new();

            let error =
                write_highlighted(&mut output, text, &spans).expect_err("invalid spans must fail");

            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert!(output.is_empty());
        }
    }

    #[test]
    fn highlighting_propagates_write_failures() {
        let text = "Rust";
        let spans = [crate::search::MatchSpan::new(text, 0, 4).expect("valid span")];
        let mut output: &mut [u8] = &mut [];

        let error = write_highlighted(&mut output, text, &spans)
            .expect_err("the destination cannot accept output");

        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    }
}
