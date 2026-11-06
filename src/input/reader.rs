use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub(crate) fn open(path: &Path) -> io::Result<BufReader<File>> {
    let file = File::open(path)?;
    Ok(BufReader::new(file))
}

pub(crate) fn numbered_lines(
    reader: impl BufRead,
) -> impl Iterator<Item = io::Result<(usize, String)>> {
    reader
        .lines()
        .enumerate()
        .map(|(index, line)| line.map(|text| (index + 1, text)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

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
}
