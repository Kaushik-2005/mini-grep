use std::io::{self, BufRead};
use std::path::PathBuf;

pub(crate) fn read(mut reader: impl BufRead) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut line_number = 0_u64;

    loop {
        let mut line = Vec::new();

        if reader.read_until(b'\n', &mut line)? == 0 {
            return Ok(paths);
        }

        line_number = line_number.saturating_add(1);

        if line.last() == Some(&b'\n') {
            line.pop();

            if line.last() == Some(&b'\r') {
                line.pop();
            }
        }

        if line.is_empty() {
            continue;
        }

        if line.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("path-list line {line_number} contains a NUL byte"),
            ));
        }

        #[cfg(unix)]
        let path = {
            use std::ffi::OsString;
            use std::os::unix::ffi::OsStringExt;

            PathBuf::from(OsString::from_vec(line))
        };

        #[cfg(not(unix))]
        let path = {
            let text = String::from_utf8(line).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("path-list line {line_number} is not valid UTF-8: {error}"),
                )
            })?;

            PathBuf::from(text)
        };

        paths.push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor, Read};

    #[test]
    fn reads_ordered_paths_with_lf_crlf_and_unterminated_last_line() -> io::Result<()> {
        let paths = read(Cursor::new("first.txt\r\nnested/second.txt\ncafé.txt"))?;

        assert_eq!(
            paths,
            vec![
                PathBuf::from("first.txt"),
                PathBuf::from("nested/second.txt"),
                PathBuf::from("café.txt"),
            ]
        );

        Ok(())
    }

    #[test]
    fn empty_input_and_blank_lines_produce_no_paths() -> io::Result<()> {
        for contents in ["", "\n", "\r\n", "\n\r\n\n"] {
            assert!(read(Cursor::new(contents))?.is_empty());
        }

        Ok(())
    }

    #[test]
    fn preserves_spaces_hash_characters_duplicates_and_literal_dash() -> io::Result<()> {
        let paths = read(Cursor::new(
            " leading and trailing \n#name\nsame\nsame\n-\n \n",
        ))?;

        let expected = [" leading and trailing ", "#name", "same", "same", "-", " "];

        assert_eq!(paths.len(), expected.len());

        for (path, expected) in paths.iter().zip(expected) {
            assert_eq!(path.as_os_str(), std::ffi::OsStr::new(expected));
        }

        Ok(())
    }

    #[test]
    fn removes_carriage_return_only_as_part_of_crlf() -> io::Result<()> {
        let paths = read(Cursor::new(b"first\r\nlast\r"))?;

        assert_eq!(
            paths,
            vec![PathBuf::from("first"), PathBuf::from("last\r"),]
        );

        Ok(())
    }

    #[test]
    fn rejects_nul_bytes_with_the_original_line_number() {
        let error = read(Cursor::new(b"\nvalid.txt\nbad\0name\n"))
            .expect_err("NUL bytes must invalidate the list");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("line 3"));
        assert!(error.to_string().contains("NUL"));
    }

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("simulated path-list read failure"))
        }
    }

    #[test]
    fn preserves_a_read_failure_after_a_complete_path() {
        let input = Cursor::new(b"valid.txt\n").chain(FailingReader);

        let error =
            read(BufReader::new(input)).expect_err("a later read failure must invalidate the load");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "simulated path-list read failure");
    }

    #[cfg(unix)]
    #[test]
    fn preserves_non_utf8_unix_filename_bytes() -> io::Result<()> {
        use std::os::unix::ffi::OsStrExt;

        let paths = read(Cursor::new(b"sample-\xff.txt\n"))?;

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].as_os_str().as_bytes(), b"sample-\xff.txt");

        Ok(())
    }

    #[cfg(not(unix))]
    #[test]
    fn rejects_non_utf8_path_entries() {
        let error = read(Cursor::new(b"valid.txt\nsample-\xff.txt\n"))
            .expect_err("path entries must be valid UTF-8 on this platform");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("line 2"));
        assert!(error.to_string().contains("UTF-8"));
    }
}
