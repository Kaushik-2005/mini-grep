use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum AppError {
    Usage(&'static str),
    Cli(Box<clap::Error>),
    Discovery(Box<ignore::Error>),
    Open { path: PathBuf, source: io::Error },
    Read { path: PathBuf, source: io::Error },
    Output(io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => write!(formatter, "{message}"),
            Self::Cli(error) => fmt::Display::fmt(error, formatter),
            Self::Discovery(error) => {
                write!(formatter, "cannot discover files: {error}")
            }
            Self::Open { path, source } => {
                write!(formatter, "cannot open '{}': {source}", path.display())
            }
            Self::Read { path, source } => {
                write!(formatter, "cannot read '{}': {source}", path.display())
            }
            Self::Output(source) => {
                write!(formatter, "cannot write output: {source}")
            }
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Usage(_) => None,
            Self::Cli(error) => Some(error.as_ref()),
            Self::Discovery(error) => Some(error.as_ref()),
            Self::Open { source, .. } | Self::Read { source, .. } | Self::Output(source) => {
                Some(source)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_error_has_no_underlying_source() {
        let error = AppError::Usage("expected a pattern and a file path");

        assert_eq!(error.to_string(), "expected a pattern and a file path");
        assert!(error.source().is_none());
    }

    #[test]
    fn file_errors_include_operation_path_and_source() {
        let cases = [
            (
                AppError::Open {
                    path: PathBuf::from("sample.txt"),
                    source: io::Error::new(io::ErrorKind::NotFound, "file missing"),
                },
                "cannot open 'sample.txt': file missing",
                "file missing",
            ),
            (
                AppError::Read {
                    path: PathBuf::from("sample.txt"),
                    source: io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8"),
                },
                "cannot read 'sample.txt': invalid UTF-8",
                "invalid UTF-8",
            ),
        ];

        for (error, expected_message, expected_source) in cases {
            assert_eq!(error.to_string(), expected_message);
            assert_eq!(
                error.source().expect("I/O source must exist").to_string(),
                expected_source
            );
        }
    }

    #[test]
    fn output_error_preserves_its_source() {
        let error = AppError::Output(io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed"));

        assert_eq!(error.to_string(), "cannot write output: pipe closed");

        let source = error
            .source()
            .expect("I/O source must exist")
            .downcast_ref::<io::Error>()
            .expect("source must retain its I/O error type");

        assert_eq!(source.kind(), io::ErrorKind::BrokenPipe);
    }
}
