use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::AppError;
use crate::input::reader;
use crate::output::terminal;
use crate::search::{self, LineMatch};

const USAGE: &str = "usage: mini-grep <PATTERN> <PATH>";

fn parse_args(mut args: impl Iterator<Item = OsString>) -> Result<(String, PathBuf), AppError> {
    let pattern = args.next().ok_or(AppError::Usage(USAGE))?;
    let path = args.next().ok_or(AppError::Usage(USAGE))?;

    if args.next().is_some() {
        return Err(AppError::Usage(USAGE));
    }

    let pattern = pattern
        .into_string()
        .map_err(|_| AppError::Usage("pattern must be valid UTF-8"))?;

    Ok((pattern, PathBuf::from(path)))
}

pub fn run() -> Result<bool, AppError> {
    let (pattern, path) = parse_args(env::args_os().skip(1))?;

    let input = reader::open(&path).map_err(|source| AppError::Open {
        path: path.clone(),
        source,
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let mut found_match = false;

    for line in reader::numbered_lines(input) {
        let (line_number, text) = line.map_err(|source| AppError::Read {
            path: path.clone(),
            source,
        })?;

        if search::matches_line(&pattern, &text) {
            let matched_line = LineMatch {
                line_number,
                text: &text,
            };

            terminal::write_match(&mut writer, &path, &matched_line).map_err(AppError::Output)?;

            found_match = true;
        }
    }

    writer.flush().map_err(AppError::Output)?;

    Ok(found_match)
}
