use std::io::{self, Write};
use std::path::Path;

use crate::search::LineMatch;

pub(crate) fn write_match(
    writer: &mut impl Write,
    path: &Path,
    matched_line: &LineMatch<'_>,
) -> io::Result<()> {
    writeln!(
        writer,
        "{}:{}:{}",
        path.display(),
        matched_line.line_number,
        matched_line.text
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_path_line_number_and_text() -> io::Result<()> {
        let mut output = Vec::new();
        let matched_line = LineMatch {
            line_number: 7,
            text: "नमस्ते Rust",
        };

        write_match(&mut output, Path::new("sample file.txt"), &matched_line)?;

        assert_eq!(output, "sample file.txt:7:नमस्ते Rust\n".as_bytes());

        Ok(())
    }

    #[test]
    fn writes_an_empty_matching_line() -> io::Result<()> {
        let mut output = Vec::new();
        let matched_line = LineMatch {
            line_number: 2,
            text: "",
        };

        write_match(&mut output, Path::new("sample.txt"), &matched_line)?;

        assert_eq!(output, b"sample.txt:2:\n");

        Ok(())
    }

    #[test]
    fn propagates_write_errors() {
        let mut output: &mut [u8] = &mut [];
        let matched_line = LineMatch {
            line_number: 1,
            text: "Rust",
        };

        let error = write_match(&mut output, Path::new("sample.txt"), &matched_line)
            .expect_err("an empty fixed-size buffer cannot accept output");

        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    }
}
