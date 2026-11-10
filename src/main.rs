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

            if let Err(source) = error.print() {
                return report_error(AppError::Output(source));
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
