use std::io::{self, BufRead};

pub(crate) fn read(reader: impl BufRead) -> io::Result<Vec<String>> {
    reader.lines().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor, Read};

    #[test]
    fn preserves_order_and_removes_crlf_endings() -> io::Result<()> {
        let patterns = read(Cursor::new("Rust\r\nGo\r\n"))?;

        assert_eq!(patterns, vec![String::from("Rust"), String::from("Go")]);

        Ok(())
    }

    #[test]
    fn preserves_blank_lines_spaces_and_hash_characters() -> io::Result<()> {
        let patterns = read(Cursor::new("\n Rust \n# heading\n\n"))?;

        assert_eq!(
            patterns,
            vec![
                String::new(),
                String::from(" Rust "),
                String::from("# heading"),
                String::new(),
            ]
        );

        Ok(())
    }

    #[test]
    fn empty_input_contributes_no_patterns() -> io::Result<()> {
        assert!(read(Cursor::new(""))?.is_empty());

        Ok(())
    }

    #[test]
    fn invalid_utf8_fails_the_entire_load() {
        let error = read(Cursor::new(b"Rust\n\xff\n")).expect_err("invalid UTF-8 must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("simulated pattern-file read failure"))
        }
    }

    #[test]
    fn read_failure_after_a_valid_pattern_is_preserved() {
        let input = Cursor::new(b"Rust\n").chain(FailingReader);

        let error =
            read(BufReader::new(input)).expect_err("a later read failure must fail the load");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "simulated pattern-file read failure");
    }

    #[test]
    fn duplicate_patterns_are_preserved() -> io::Result<()> {
        let patterns = read(Cursor::new("Rust\nRust\n"))?;

        assert_eq!(patterns, vec![String::from("Rust"), String::from("Rust")]);

        Ok(())
    }
}
