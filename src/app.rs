use std::io::{self, Write};

use crate::AppError;
use crate::cli;
use crate::input::reader;
use crate::output::terminal;
use crate::search::{CaseSensitivity, LineMatch, LiteralMatcher, Selection};

pub fn run() -> Result<bool, AppError> {
    let config = cli::parse()?;

    let case = if config.ignore_case {
        CaseSensitivity::Insensitive
    } else {
        CaseSensitivity::Sensitive
    };

    let selection = if config.invert_match {
        Selection::NonMatching
    } else {
        Selection::Matching
    };

    let matcher = LiteralMatcher::new(&config.pattern, case, selection);

    let input = reader::open(&config.path).map_err(|source| AppError::Open {
        path: config.path.clone(),
        source,
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let mut found_selected_line = false;

    for line in reader::numbered_lines(input) {
        let (line_number, text) = line.map_err(|source| AppError::Read {
            path: config.path.clone(),
            source,
        })?;

        if matcher.is_selected(&text) {
            let matched_line = LineMatch {
                line_number,
                text: &text,
            };

            terminal::write_match(
                &mut writer,
                &config.path,
                &matched_line,
                config.line_numbers,
            )
            .map_err(AppError::Output)?;

            found_selected_line = true;
        }
    }

    writer.flush().map_err(AppError::Output)?;

    Ok(found_selected_line)
}
