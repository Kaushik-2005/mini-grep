use std::ffi::OsString;
use std::path::PathBuf;

use clap::{ArgAction, Parser};

#[derive(Debug, Parser)]
#[command(
    name = "mini-grep",
    version,
    about = "Search UTF-8 files in a file or directory.",
    override_usage = "mini-grep [OPTIONS] <PATTERN> <PATH>\n       mini-grep [OPTIONS] (-e <PATTERN> | -f <FILE>)... <PATH>",
    disable_help_flag = true
)]
pub(crate) struct Args {
    /// Pattern and path, or path only when -e/-f supplies patterns
    #[arg(value_name = "INPUT", num_args = 0..)]
    pub(crate) inputs: Vec<OsString>,

    /// Use Unicode-aware case-insensitive matching
    #[arg(short = 'i', long, conflicts_with = "smart_case")]
    pub(crate) ignore_case: bool,

    /// Treat the pattern as literal text
    #[arg(short = 'F', long)]
    pub(crate) fixed_strings: bool,

    /// Include 1-based line numbers in output
    #[arg(short = 'n', long = "line-number")]
    pub(crate) line_numbers: bool,

    /// Select lines that do not match
    #[arg(short = 'v', long)]
    pub(crate) invert_match: bool,

    /// Include hidden files and directories
    #[arg(long)]
    pub(crate) hidden: bool,

    /// Disable all supported ignore-file rules
    #[arg(long)]
    pub(crate) no_ignore: bool,

    /// Disable Git ignore rules while retaining .ignore rules
    #[arg(long)]
    pub(crate) no_ignore_vcs: bool,

    /// Maximum traversal depth, with the supplied root at depth 0
    #[arg(long, value_name = "DEPTH")]
    pub(crate) max_depth: Option<usize>,

    /// Maximum searched file size in bytes, inclusive
    #[arg(long, value_name = "BYTES")]
    pub(crate) max_filesize: Option<u64>,

    /// Include matching paths; prefix with ! to exclude; later matches win
    #[arg(short = 'g', long = "glob", value_name = "GLOB")]
    pub(crate) globs: Vec<String>,

    /// Include a built-in file type; repeat to include more types
    #[arg(long = "type", value_name = "TYPE")]
    pub(crate) file_types: Vec<String>,

    /// Exclude a built-in file type; exclusions take precedence
    #[arg(long = "type-not", value_name = "TYPE")]
    pub(crate) excluded_types: Vec<String>,

    /// Follow symbolic links and report broken links or loops
    #[arg(short = 'L', long)]
    pub(crate) follow: bool,

    /// Ignore case when no pattern contains an uppercase character
    #[arg(short = 'S', long)]
    pub(crate) smart_case: bool,

    /// Require non-word characters or text boundaries around matches
    #[arg(short = 'w', long, conflicts_with = "line_regexp")]
    pub(crate) word_regexp: bool,

    /// Require a match covering the entire line
    #[arg(short = 'x', long)]
    pub(crate) line_regexp: bool,

    /// Add a search pattern; repeat to add more
    #[arg(short = 'e', long = "regexp", value_name = "PATTERN")]
    pub(crate) patterns: Vec<String>,

    /// Read patterns from a UTF-8 file; repeat to add more files
    #[arg(short = 'f', long = "file", value_name = "FILE")]
    pub(crate) pattern_files: Vec<PathBuf>,

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

        assert_eq!(
            args.inputs,
            vec![OsString::from("Rust"), OsString::from("sample.txt")]
        );
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
    fn parses_explicit_patterns_with_a_path_only() {
        let args = Args::try_parse_from(["mini-grep", "-e", "Rust", "sample.txt"])
            .expect("explicit pattern syntax must parse");

        assert_eq!(args.patterns, vec![String::from("Rust")]);
        assert_eq!(args.inputs, vec![OsString::from("sample.txt")]);
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

        assert_eq!(args.inputs[0], OsString::from("-needle"));
    }

    #[test]
    fn accepts_an_empty_pattern() {
        let args = Args::try_parse_from(["mini-grep", "", "sample.txt"])
            .expect("an empty pattern is valid");

        assert!(args.inputs[0].is_empty());
    }

    #[test]
    fn rejects_conflicting_case_flags_in_either_order() {
        for flags in ["-iS", "-Si"] {
            let error = Args::try_parse_from(["mini-grep", flags, "Rust", "sample.txt"])
                .expect_err("case flags must conflict");

            assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
        }
    }

    #[test]
    fn rejects_conflicting_scope_flags_in_either_order() {
        for flags in ["-wx", "-xw"] {
            let error = Args::try_parse_from(["mini-grep", flags, "Rust", "sample.txt"])
                .expect_err("scope flags must conflict");

            assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
        }
    }
}
