use std::io::{self, Write};
use std::path::Path;

use crate::AppError;
use crate::search::LineMatch;

pub(crate) fn write_match(
    writer: &mut impl Write,
    path: &Path,
    matched_line: &LineMatch<'_>,
    line_numbers: bool,
) -> io::Result<()> {
    if line_numbers {
        writeln!(
            writer,
            "{}:{}:{}",
            path.display(),
            matched_line.line_number,
            matched_line.text
        )
    } else {
        writeln!(writer, "{}:{}", path.display(), matched_line.text)
    }
}

pub(crate) fn write_diagnostic(writer: &mut impl Write, error: &AppError) -> io::Result<()> {
    writeln!(writer, "mini-grep: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_path_line_number_and_text_when_enabled() -> io::Result<()> {
        let mut output = Vec::new();
        let matched_line = LineMatch {
            line_number: 7,
            text: "नमस्ते Rust",
        };

        write_match(
            &mut output,
            Path::new("sample file.txt"),
            &matched_line,
            true,
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
        };

        write_match(&mut output, Path::new("sample.txt"), &matched_line, false)?;

        assert_eq!(output, b"sample.txt:Rust\n");

        Ok(())
    }

    #[test]
    fn writes_an_empty_matching_line() -> io::Result<()> {
        let mut output = Vec::new();
        let matched_line = LineMatch {
            line_number: 2,
            text: "",
        };

        write_match(&mut output, Path::new("sample.txt"), &matched_line, false)?;

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
            };

            let error = write_match(
                &mut output,
                Path::new("sample.txt"),
                &matched_line,
                line_numbers,
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
}
