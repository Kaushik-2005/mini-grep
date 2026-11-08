use std::path::PathBuf;

use clap::{ArgAction, Parser};

#[derive(Debug, Parser)]
#[command(
    name = "mini-grep",
    version,
    about = "Search one UTF-8 file for literal text.",
    disable_help_flag = true
)]
pub(crate) struct Args {
    /// Literal text to search for
    #[arg(value_name = "PATTERN")]
    pub(crate) pattern: String,

    /// File to search
    #[arg(value_name = "PATH")]
    pub(crate) path: PathBuf,

    /// Match after Unicode lowercasing
    #[arg(short = 'i', long)]
    pub(crate) ignore_case: bool,

    /// Treat the pattern as literal text (currently the default)
    #[arg(short = 'F', long)]
    pub(crate) fixed_strings: bool,

    /// Include 1-based line numbers in output
    #[arg(short = 'n', long = "line-number")]
    pub(crate) line_numbers: bool,

    /// Select lines that do not match
    #[arg(short = 'v', long)]
    pub(crate) invert_match: bool,

    /// Print help
    #[arg(long, action = ArgAction::Help)]
    pub(crate) help: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::error::ErrorKind;

    #[test]
    fn parser_definition_is_valid() {
        Args::command().debug_assert();
    }

    #[test]
    fn parses_positionals_with_default_settings() {
        let args = Args::try_parse_from(["mini-grep", "Rust", "sample.txt"])
            .expect("valid arguments must parse");

        assert_eq!(args.pattern, "Rust");
        assert_eq!(args.path, PathBuf::from("sample.txt"));
        assert!(!args.ignore_case);
        assert!(!args.fixed_strings);
        assert!(!args.line_numbers);
        assert!(!args.invert_match);
    }

    #[test]
    fn accepts_combined_search_flags() {
        let args = Args::try_parse_from(["mini-grep", "-iFnv", "Rust", "sample.txt"])
            .expect("compatible flags must parse");

        assert!(args.ignore_case);
        assert!(args.fixed_strings);
        assert!(args.line_numbers);
        assert!(args.invert_match);
    }

    #[test]
    fn requires_a_file_path() {
        let error =
            Args::try_parse_from(["mini-grep", "Rust"]).expect_err("a missing path must fail");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn rejects_unknown_flags() {
        let error = Args::try_parse_from(["mini-grep", "--unknown", "Rust", "sample.txt"])
            .expect_err("an unknown flag must fail");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn reserves_short_h_instead_of_using_it_for_help() {
        let error =
            Args::try_parse_from(["mini-grep", "-h"]).expect_err("-h is not implemented yet");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn help_does_not_require_search_arguments() {
        let error = Args::try_parse_from(["mini-grep", "--help"])
            .expect_err("help is returned as a display request");

        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        assert_eq!(error.exit_code(), 0);
        assert!(!error.use_stderr());
    }

    #[test]
    fn version_does_not_require_search_arguments() {
        let error = Args::try_parse_from(["mini-grep", "--version"])
            .expect_err("version is returned as a display request");

        assert_eq!(error.kind(), ErrorKind::DisplayVersion);
        assert_eq!(error.exit_code(), 0);
        assert!(!error.use_stderr());
    }

    #[test]
    fn accepts_a_hyphenated_pattern_after_the_separator() {
        let args = Args::try_parse_from(["mini-grep", "--", "-needle", "sample.txt"])
            .expect("the separator must allow a hyphenated pattern");

        assert_eq!(args.pattern, "-needle");
    }

    #[test]
    fn accepts_an_empty_pattern() {
        let args = Args::try_parse_from(["mini-grep", "", "sample.txt"])
            .expect("an empty pattern is valid");

        assert!(args.pattern.is_empty());
    }
}
