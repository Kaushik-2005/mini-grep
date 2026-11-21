use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use crate::execution::cancel::CancellationToken;

pub(crate) fn open(path: &Path) -> io::Result<BufReader<File>> {
    let file = File::open(path)?;
    // Ok(BufReader::new(file))
    Ok(BufReader::with_capacity(64 * 1024, file))
}

pub(crate) struct CancellationReader<'a, R> {
    inner: R,
    cancellation: &'a CancellationToken,
}

impl<'a, R> CancellationReader<'a, R> {
    pub(crate) fn new(inner: R, cancellation: &'a CancellationToken) -> Self {
        Self {
            inner,
            cancellation,
        }
    }

    fn check(&self) -> io::Result<()> {
        if self.cancellation.is_cancelled() {
            Err(io::Error::other("operation cancelled"))
        } else {
            Ok(())
        }
    }
}

impl<R: Read> Read for CancellationReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.check()?;
        self.inner.read(buffer)
    }
}

impl<R: BufRead> BufRead for CancellationReader<'_, R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.check()?;
        self.inner.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.inner.consume(amount);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InputLine {
    Text { line_number: usize, text: String },
    Binary,
}

pub(crate) struct ClassifiedLines<R> {
    reader: R,
    policy: crate::input::binary::BinaryPolicy,
    bytes: Vec<u8>,
    line_number: usize,
    stopped: bool,
}

pub(crate) fn classified_lines(
    reader: impl BufRead,
    policy: crate::input::binary::BinaryPolicy,
) -> ClassifiedLines<impl BufRead> {
    ClassifiedLines {
        reader,
        policy,
        bytes: Vec::new(),
        line_number: 0,
        stopped: false,
    }
}

impl<R> ClassifiedLines<R> {
    /// Returns a discarded line's allocation for the next read.
    pub(crate) fn recycle(&mut self, text: String) {
        self.bytes = text.into_bytes();
        self.bytes.clear();
    }
}

impl<R: BufRead> Iterator for ClassifiedLines<R> {
    type Item = io::Result<InputLine>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }

        match self.reader.read_until(b'\n', &mut self.bytes) {
            Ok(0) => {
                self.stopped = true;
                return None;
            }
            Ok(_) => {}
            Err(error) => {
                self.stopped = true;
                return Some(Err(error));
            }
        }

        if self.policy.should_stop(&self.bytes) {
            self.stopped = true;
            return Some(Ok(InputLine::Binary));
        }

        if self.bytes.last() == Some(&b'\n') {
            self.bytes.pop();

            if self.bytes.last() == Some(&b'\r') {
                self.bytes.pop();
            }
        }

        self.line_number = match self.line_number.checked_add(1) {
            Some(number) => number,
            None => {
                self.stopped = true;
                return Some(Err(io::Error::other("line number exceeds usize")));
            }
        };

        let bytes = std::mem::take(&mut self.bytes);

        match String::from_utf8(bytes) {
            Ok(text) => Some(Ok(InputLine::Text {
                line_number: self.line_number,
                text,
            })),
            Err(error) => {
                self.stopped = true;
                Some(Err(io::Error::new(io::ErrorKind::InvalidData, error)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    pub(crate) fn numbered_lines(
        reader: impl BufRead,
    ) -> impl Iterator<Item = io::Result<(usize, String)>> {
        reader
            .lines()
            .enumerate()
            .map(|(index, line)| line.map(|text| (index + 1, text)))
    }

    #[test]
    fn reads_numbered_lines_and_removes_line_endings() -> io::Result<()> {
        let input = Cursor::new("first\r\n\r\nनमस्ते\nlast");
        let lines = numbered_lines(input).collect::<io::Result<Vec<_>>>()?;

        assert_eq!(
            lines,
            vec![
                (1, String::from("first")),
                (2, String::new()),
                (3, String::from("नमस्ते")),
                (4, String::from("last")),
            ]
        );

        Ok(())
    }

    #[test]
    fn empty_input_has_no_lines() {
        let mut lines = numbered_lines(Cursor::new(""));
        assert!(lines.next().is_none());
    }

    #[test]
    fn trailing_newline_does_not_add_an_extra_line() -> io::Result<()> {
        let lines = numbered_lines(Cursor::new("first\n")).collect::<io::Result<Vec<_>>>()?;

        assert_eq!(lines, vec![(1, String::from("first"))]);

        Ok(())
    }

    #[test]
    fn invalid_utf8_returns_a_read_error() {
        let input = Cursor::new(vec![0xff, b'\n']);
        let result = numbered_lines(input).collect::<io::Result<Vec<_>>>();

        let error = result.expect_err("invalid UTF-8 must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("simulated read failure"))
        }
    }

    #[test]
    fn reports_a_read_failure_after_a_complete_line() -> io::Result<()> {
        let input = Cursor::new(b"first\n").chain(FailingReader);
        let mut lines = numbered_lines(BufReader::new(input));

        let first = lines.next().expect("the first line must exist")?;
        assert_eq!(first, (1, String::from("first")));

        let error = lines
            .next()
            .expect("the read error must be yielded")
            .expect_err("the second read must fail");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "simulated read failure");

        Ok(())
    }

    #[test]
    fn opens_and_reads_the_existing_fixture() -> io::Result<()> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/text/demo.txt");

        let reader = open(&path)?;
        let lines = numbered_lines(reader).collect::<io::Result<Vec<_>>>()?;

        assert_eq!(lines.len(), 4);
        assert_eq!(
            lines[1],
            (2, String::from("Rust makes ownership explicit."))
        );

        Ok(())
    }

    #[test]
    fn cancelled_adapter_does_not_consume_input() {
        let cancellation = CancellationToken::default();
        cancellation.cancel();

        let mut input = Cursor::new(b"Rust\n");

        {
            let mut reader = CancellationReader::new(&mut input, &cancellation);
            let mut buffer = [0_u8; 1];

            assert!(reader.read(&mut buffer).is_err());
            assert!(reader.fill_buf().is_err());
        }

        assert_eq!(input.position(), 0);
    }

    #[test]
    fn adapter_preserves_lines_until_cancellation() {
        let cancellation = CancellationToken::default();
        let input = Cursor::new(b"Rust\r\nGo\n");
        let mut reader = CancellationReader::new(input, &cancellation);
        let mut line = String::new();

        assert_eq!(
            reader.read_line(&mut line).expect("first line must read"),
            6
        );
        assert_eq!(line, "Rust\r\n");

        cancellation.cancel();
        line.clear();

        assert!(reader.read_line(&mut line).is_err());
        assert!(line.is_empty());
    }

    #[test]
    fn classified_reader_preserves_numbering_and_line_endings() -> io::Result<()> {
        use crate::input::binary::BinaryPolicy;

        let input = Cursor::new("first\r\n\nनमस्ते\nlast\r");
        let lines = classified_lines(input, BinaryPolicy::Skip).collect::<io::Result<Vec<_>>>()?;

        assert_eq!(
            lines,
            vec![
                InputLine::Text {
                    line_number: 1,
                    text: String::from("first"),
                },
                InputLine::Text {
                    line_number: 2,
                    text: String::new(),
                },
                InputLine::Text {
                    line_number: 3,
                    text: String::from("नमस्ते"),
                },
                InputLine::Text {
                    line_number: 4,
                    text: String::from("last\r"),
                },
            ],
        );
        Ok(())
    }

    #[test]
    fn classified_reader_stops_at_binary_before_decoding() -> io::Result<()> {
        use crate::input::binary::BinaryPolicy;

        let input = Cursor::new(b"Rust\n\xff\0\nlater\n");
        let mut lines = classified_lines(input, BinaryPolicy::Skip);

        assert_eq!(
            lines.next().expect("first line must exist")?,
            InputLine::Text {
                line_number: 1,
                text: String::from("Rust"),
            },
        );
        assert_eq!(
            lines.next().expect("binary event must exist")?,
            InputLine::Binary,
        );
        assert!(lines.next().is_none());
        Ok(())
    }

    #[test]
    fn text_policy_preserves_nul_in_valid_utf8() -> io::Result<()> {
        use crate::input::binary::BinaryPolicy;

        let lines = classified_lines(Cursor::new(b"Ru\0st\n"), BinaryPolicy::Text)
            .collect::<io::Result<Vec<_>>>()?;

        assert_eq!(
            lines,
            vec![InputLine::Text {
                line_number: 1,
                text: String::from("Ru\0st"),
            }],
        );
        Ok(())
    }

    #[test]
    fn classified_reader_rejects_invalid_utf8_without_nul() {
        use crate::input::binary::BinaryPolicy;

        for policy in [BinaryPolicy::Skip, BinaryPolicy::Text] {
            let mut lines = classified_lines(Cursor::new(b"\xff\n"), policy);

            let error = lines
                .next()
                .expect("error event must exist")
                .expect_err("invalid UTF-8 must fail");

            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert!(lines.next().is_none());
        }
    }

    #[test]
    fn text_policy_does_not_bypass_utf8_validation_with_nul() {
        use crate::input::binary::BinaryPolicy;

        let mut lines = classified_lines(Cursor::new(b"\xff\0\n"), BinaryPolicy::Text);

        let error = lines
            .next()
            .expect("error event must exist")
            .expect_err("text policy must still require valid UTF-8");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(lines.next().is_none());
    }

    #[test]
    fn recycled_buffer_preserves_capacity_and_clears_old_text() -> io::Result<()> {
        use crate::input::binary::BinaryPolicy;

        let input = Cursor::new(b"a longer first line\r\nx\n\nlast");
        let mut lines = classified_lines(input, BinaryPolicy::Skip);

        for (index, expected) in ["a longer first line", "x", "", "last"]
            .into_iter()
            .enumerate()
        {
            let line = lines.next().expect("line must exist")?;

            let InputLine::Text { line_number, text } = line else {
                panic!("text input must produce a text line");
            };

            assert_eq!(line_number, index + 1);
            assert_eq!(text, expected);

            let capacity = text.capacity();
            lines.recycle(text);

            assert!(lines.bytes.is_empty());
            assert_eq!(lines.bytes.capacity(), capacity);
        }

        assert!(lines.next().is_none());
        assert!(lines.next().is_none());

        Ok(())
    }
}
