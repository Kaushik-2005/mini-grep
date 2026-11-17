use std::io::{self, Write};
use std::process::ExitCode;

use mini_grep::{AppError, SearchOutcome};

fn main() -> ExitCode {
    match mini_grep::run() {
        Ok(SearchOutcome::Selected) => ExitCode::SUCCESS,
        Ok(SearchOutcome::NoSelection) => ExitCode::from(1),
        Ok(SearchOutcome::Errors) => ExitCode::from(2),
        Err(AppError::Cli(error)) => {
            let code = if error.exit_code() == 0 { 0 } else { 2 };
            let use_stderr = error.use_stderr();

            if let Err(source) = error.print() {
                return match classify_cli_write_error(source, use_stderr) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(error) => report_error(error),
                };
            }

            ExitCode::from(code)
        }
        Err(error) => report_error(error),
    }
}

fn report_error(error: AppError) -> ExitCode {
    let mut stderr = io::stderr().lock();
    let _ = writeln!(stderr, "mini-grep: {error}");
    ExitCode::from(2)
}

fn classify_cli_write_error(source: io::Error, use_stderr: bool) -> Result<(), AppError> {
    if use_stderr {
        Err(AppError::Diagnostic(source))
    } else if source.kind() == io::ErrorKind::BrokenPipe {
        Ok(())
    } else {
        Err(AppError::Output(source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_help_or_version_stdout_is_graceful() {
        let result = classify_cli_write_error(
            io::Error::new(io::ErrorKind::BrokenPipe, "consumer closed"),
            false,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn cli_stderr_failures_remain_diagnostic_errors() {
        for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::PermissionDenied] {
            let error = classify_cli_write_error(io::Error::new(kind, "diagnostic failed"), true)
                .expect_err("stderr failures must remain errors");

            match error {
                AppError::Diagnostic(source) => assert_eq!(source.kind(), kind),
                other => panic!("expected diagnostic error, got {other:?}"),
            }
        }
    }

    #[test]
    fn other_cli_stdout_failures_remain_output_errors() {
        let error = classify_cli_write_error(
            io::Error::new(io::ErrorKind::PermissionDenied, "output denied"),
            false,
        )
        .expect_err("ordinary stdout failures must remain errors");

        match error {
            AppError::Output(source) => {
                assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
            }
            other => panic!("expected output error, got {other:?}"),
        }
    }
}
