use std::io::{self, Write};
use std::path::Path;

use crate::AppError;
use crate::cli;
use crate::discovery::walker;
use crate::input::reader;
use crate::output::terminal;
use crate::search::{CaseSensitivity, LineMatch, LiteralMatcher, Selection};

#[derive(Debug)]
pub enum SearchOutcome {
    Selected,
    NoSelection,
    Errors,
}

pub fn run() -> Result<SearchOutcome, AppError> {
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

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let stderr = io::stderr();
    let mut diagnostics = stderr.lock();

    let mut found_selected_line = false;
    let mut had_errors = false;

    let candidates = walker::files(&config.path, &config.discovery)
        .map_err(|error| AppError::Discovery(Box::new(error)))?;

    for candidate in candidates {
        let result = candidate
            .map_err(|error| AppError::Discovery(Box::new(error)))
            .and_then(|path| search_file(&path, &matcher, config.line_numbers, &mut writer));

        match result {
            Ok(selected) => {
                found_selected_line |= selected;
            }
            Err(error) => {
                if matches!(&error, AppError::Output(_)) {
                    return Err(error);
                }

                terminal::write_diagnostic(&mut diagnostics, &error).map_err(AppError::Output)?;

                had_errors = true;
            }
        }
    }

    writer.flush().map_err(AppError::Output)?;
    diagnostics.flush().map_err(AppError::Output)?;

    if had_errors {
        Ok(SearchOutcome::Errors)
    } else if found_selected_line {
        Ok(SearchOutcome::Selected)
    } else {
        Ok(SearchOutcome::NoSelection)
    }
}

fn search_file(
    path: &Path,
    matcher: &LiteralMatcher,
    line_numbers: bool,
    writer: &mut impl Write,
) -> Result<bool, AppError> {
    let input = reader::open(path).map_err(|source| AppError::Open {
        path: path.to_path_buf(),
        source,
    })?;

    let mut found_selected_line = false;

    for line in reader::numbered_lines(input) {
        let (line_number, text) = line.map_err(|source| AppError::Read {
            path: path.to_path_buf(),
            source,
        })?;

        if matcher.is_selected(&text) {
            let matched_line = LineMatch {
                line_number,
                text: &text,
            };

            terminal::write_match(writer, path, &matched_line, line_numbers)
                .map_err(AppError::Output)?;

            found_selected_line = true;
        }
    }

    Ok(found_selected_line)
}
