use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match mini_grep::run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            let mut stderr = io::stderr().lock();
            let _ = writeln!(stderr, "mini-grep: {error}");
            ExitCode::from(2)
        }
    }
}
