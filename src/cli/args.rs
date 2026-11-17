use std::ffi::OsString;
use std::path::PathBuf;

use clap::{ArgAction, Parser, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ColorArg {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Parser)]
#[command(
    name = "mini-grep",
    version,
    about = "Search UTF-8 files in a file or directory.",
    override_usage = "mini-grep [OPTIONS] <PATTERN> [PATH]\n       mini-grep [OPTIONS] (-e <PATTERN> | -f <FILE>)... [PATH]\n       mini-grep --files [OPTIONS] [PATH]",
    disable_help_flag = true
)]
pub(crate) struct Args {
    /// Pattern and optional path, or optional path when -e/-f supplies patterns
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

    /// Include this many lines after each selected line
    #[arg(short = 'A', long, value_name = "NUM")]
    pub(crate) after_context: Option<usize>,

    /// Include this many lines before each selected line
    #[arg(short = 'B', long, value_name = "NUM")]
    pub(crate) before_context: Option<usize>,

    /// Include this many lines before and after each selected line
    #[arg(short = 'C', long, value_name = "NUM")]
    pub(crate) context: Option<usize>,

    /// Stop selecting lines in each file after this count
    #[arg(short = 'm', long, value_name = "NUM")]
    pub(crate) max_count: Option<usize>,

    /// Stop selecting lines across the search after this count
    #[arg(long, value_name = "NUM")]
    pub(crate) max_results: Option<usize>,

    /// Print no results and stop at the first selected line
    #[arg(short = 'q', long)]
    pub(crate) quiet: bool,

    /// Print each file path once if it contains a selected line
    #[arg(short = 'l', long, conflicts_with = "files_without_match")]
    pub(crate) files_with_matches: bool,

    /// Print each file path once if it contains no selected lines
    #[arg(long)]
    pub(crate) files_without_match: bool,

    /// Print the number of selected lines in each file
    #[arg(
        short = 'c',
        long,
        conflicts_with_all = [
            "count_matches",
            "files_with_matches",
            "files_without_match",
        ]
    )]
    pub(crate) count: bool,

    /// Print the number of match spans in each files
    #[arg(
        long,
        conflicts_with_all = [
            "files_with_matches",
            "files_without_match",
            "invert_match",
        ]
    )]
    pub(crate) count_matches: bool,

    /// Print each nonempty matched span instead of the full time
    #[arg(
        short = 'o',
        long,
        conflicts_with_all = [
            "files_with_matches",
            "files_without_match",
            "count",
            "count_matches",
            "invert_match",
        ]
    )]
    pub(crate) only_matching: bool,

    /// Include filenames in line and count output
    #[arg(short = 'H', long, conflicts_with = "no_filename")]
    pub(crate) with_filename: bool,

    /// Omit filenames from line and count output
    #[arg(short = 'h', long)]
    pub(crate) no_filename: bool,

    /// Group line and match-only results beneath each file path
    #[arg(long, conflicts_with = "no_filename")]
    pub(crate) heading: bool,

    /// Include one-based byte columns and line numbers
    #[arg(long)]
    pub(crate) column: bool,

    /// Control match highlighting
    #[arg(long, value_enum, default_value = "auto", value_name = "WHEN")]
    pub(crate) color: ColorArg,

    /// Write a NUL byte after each emitted filename
    #[arg(short = '0', long, conflicts_with = "heading")]
    pub(crate) null: bool,

    /// List discovered files without searching their contents
    #[arg(
        long,
        conflicts_with_all = [
            "patterns",
            "pattern_files",
            "ignore_case",
            "smart_case",
            "fixed_strings",
            "word_regexp",
            "line_regexp",
            "invert_match",
            "after_context",
            "before_context",
            "context",
            "max_count",
            "max_results",
            "files_with_matches",
            "files_without_match",
            "count",
            "count_matches",
            "only_matching",
        ]
    )]
    pub(crate) files: bool,

    /// Read target paths from a file or - for stdin; repeat for more lists
    #[arg(long, value_name = "FILE")]
    pub(crate) files_from: Vec<PathBuf>,

    /// Search NUL-containing content as text; UTF-8 is still required
    #[arg(long, conflicts_with = "files")]
    pub(crate) text: bool,

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

    #[test]
    fn short_h_controls_filenames_instead_of_displaying_help() {
        let args = Args::try_parse_from(["mini-grep", "-h", "Rust", "sample.txt"])
            .expect("-h must parse as a filename option");

        assert!(args.no_filename);
        assert!(!args.with_filename);
        assert_eq!(args.help, None);
    }

    #[test]
    fn color_defaults_to_auto() {
        let args = Args::try_parse_from(["mini-grep", "Rust", "sample.txt"])
            .expect("default color settings must parse");

        assert_eq!(args.color, ColorArg::Auto);
    }

    #[test]
    fn accepts_each_color_choice() {
        for (value, expected) in [
            ("auto", ColorArg::Auto),
            ("always", ColorArg::Always),
            ("never", ColorArg::Never),
        ] {
            let args = Args::try_parse_from(["mini-grep", "--color", value, "Rust", "sample.txt"])
                .expect("supported color choice must parse");

            assert_eq!(args.color, expected);
        }
    }

    #[test]
    fn accepts_color_with_equals_syntax() {
        let args = Args::try_parse_from(["mini-grep", "--color=always", "Rust", "sample.txt"])
            .expect("equals syntax must parse");

        assert_eq!(args.color, ColorArg::Always);
    }

    #[test]
    fn rejects_invalid_color_choices() {
        for value in ["sometimes", "ALWAYS", ""] {
            let error = Args::try_parse_from(["mini-grep", "--color", value, "Rust", "sample.txt"])
                .expect_err("unsupported color choice must fail");

            assert_eq!(error.kind(), ErrorKind::InvalidValue);
        }
    }

    #[test]
    fn color_requires_a_value() {
        assert!(Args::try_parse_from(["mini-grep", "Rust", "sample.txt", "--color"]).is_err());
    }

    #[test]
    fn enumeration_accepts_a_color_choice() {
        let args = Args::try_parse_from(["mini-grep", "--files", "--color=always", "."])
            .expect("enumeration must accept color settings");

        assert!(args.files);
        assert_eq!(args.color, ColorArg::Always);
    }
}
