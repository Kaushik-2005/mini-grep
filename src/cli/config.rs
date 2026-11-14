use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::args::Args;
use crate::AppError;
use crate::discovery::DiscoveryOptions;
use crate::output::OutputMode;
use crate::search::{CaseMode, MatchScope, PatternMode, PatternOptions, Selection};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PatternSource {
    Argument(String),
    File(PathBuf),
}

#[derive(Debug)]
pub(crate) struct SearchInputs {
    pub(crate) sources: Vec<PatternSource>,
    pub(crate) path: PathBuf,
}

pub(crate) fn resolve_inputs(
    positionals: Vec<OsString>,
    explicit_patterns: Vec<String>,
    pattern_files: Vec<PathBuf>,
) -> Result<SearchInputs, AppError> {
    for path in &pattern_files {
        if path.as_os_str().is_empty() {
            return Err(AppError::Usage("pattern file path must not be empty"));
        }

        if path == Path::new("-") {
            return Err(AppError::Usage(
                "reading patterns from stdin is not supported yet",
            ));
        }
    }

    let has_explicit_sources = !explicit_patterns.is_empty() || !pattern_files.is_empty();

    let mut sources: Vec<_> = explicit_patterns
        .into_iter()
        .map(PatternSource::Argument)
        .chain(pattern_files.into_iter().map(PatternSource::File))
        .collect();

    let mut positionals = positionals.into_iter();
    let first = positionals.next();
    let second = positionals.next();
    let third = positionals.next();

    let path = match (has_explicit_sources, first, second, third) {
        (false, Some(pattern), Some(path), None) => {
            let pattern = pattern
                .into_string()
                .map_err(|_| AppError::Usage("pattern must be valid UTF-8"))?;

            sources.push(PatternSource::Argument(pattern));
            path
        }
        (true, Some(path), None, None) => path,
        _ => {
            return Err(AppError::Usage(
                "expected <PATTERN> <PATH>, or one <PATH> with -e/-f",
            ));
        }
    };

    let path = PathBuf::from(path);

    if path.as_os_str().is_empty() {
        return Err(AppError::Usage("file path must not be empty"));
    }

    Ok(SearchInputs { sources, path })
}

#[derive(Debug)]
pub(crate) struct Config {
    pub(crate) sources: Vec<PatternSource>,
    pub(crate) path: PathBuf,
    pub(crate) pattern_options: PatternOptions,
    pub(crate) line_numbers: bool,
    pub(crate) discovery: DiscoveryOptions,
    pub(crate) before_context: usize,
    pub(crate) after_context: usize,
    pub(crate) max_count: Option<usize>,
    pub(crate) max_results: Option<usize>,
    pub(crate) quiet: bool,
    pub(crate) output_mode: OutputMode,
    pub(crate) show_filename: bool,
    pub(crate) column: bool,
    pub(crate) heading: bool,
    pub(crate) null: bool,
}

impl TryFrom<Args> for Config {
    type Error = AppError;

    fn try_from(args: Args) -> Result<Self, Self::Error> {
        if args.ignore_case && args.smart_case {
            return Err(AppError::Usage("-i and -S cannot be combined"));
        }

        if args.word_regexp && args.line_regexp {
            return Err(AppError::Usage("-w and -x cannot be combined"));
        }

        if args.with_filename && args.no_filename {
            return Err(AppError::Usage(
                "--with-filename and --no-filename cannot be combined",
            ));
        }

        if args.heading && args.no_filename {
            return Err(AppError::Usage(
                "--heading and --no-filename cannot be combined",
            ));
        }

        if args.null && args.heading {
            return Err(AppError::Usage("--null and --heading cannot be combined"));
        }

        let requested_modes = [
            args.files_with_matches,
            args.files_without_match,
            args.count,
            args.count_matches,
            args.only_matching,
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();

        if requested_modes > 1 {
            return Err(AppError::Usage(
                "file-list, count, and match-only output modes cannot be combined",
            ));
        }

        if args.count_matches && args.invert_match {
            return Err(AppError::Usage(
                "--count-matches and --invert-match cannot be combined",
            ));
        }

        if args.only_matching && args.invert_match {
            return Err(AppError::Usage(
                "--only-matching and --invert-match cannot be combined",
            ));
        }

        let output_mode = if args.files_with_matches {
            OutputMode::FilesWithMatches
        } else if args.files_without_match {
            OutputMode::FilesWithoutMatch
        } else if args.count {
            OutputMode::CountLines
        } else if args.count_matches {
            OutputMode::CountMatches
        } else if args.only_matching {
            OutputMode::OnlyMatching
        } else {
            OutputMode::Lines
        };

        let inputs = resolve_inputs(args.inputs, args.patterns, args.pattern_files)?;

        let case = if args.ignore_case {
            CaseMode::Insensitive
        } else if args.smart_case {
            CaseMode::Smart
        } else {
            CaseMode::Sensitive
        };

        let scope = if args.word_regexp {
            MatchScope::Word
        } else if args.line_regexp {
            MatchScope::Line
        } else {
            MatchScope::Substring
        };

        Ok(Self {
            sources: inputs.sources,
            path: inputs.path,
            pattern_options: PatternOptions {
                mode: if args.fixed_strings {
                    PatternMode::Literal
                } else {
                    PatternMode::Regex
                },
                case,
                scope,
                selection: if args.invert_match {
                    Selection::NonMatching
                } else {
                    Selection::Matching
                },
            },
            line_numbers: args.line_numbers || args.column,
            column: args.column,
            show_filename: !args.no_filename,
            heading: args.heading,
            null: args.null,
            quiet: args.quiet,
            output_mode,
            before_context: args.before_context.or(args.context).unwrap_or(0),
            after_context: args.after_context.or(args.context).unwrap_or(0),
            max_count: args.max_count,
            max_results: args.max_results,
            discovery: DiscoveryOptions {
                include_hidden: args.hidden,
                ignore_files: !args.no_ignore,
                vcs_ignore_files: !(args.no_ignore || args.no_ignore_vcs),
                max_depth: args.max_depth,
                max_filesize: args.max_filesize,
                globs: args.globs,
                file_types: args.file_types,
                excluded_types: args.excluded_types,
                follow_links: args.follow,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn converts_default_settings() {
        let args = Args::try_parse_from(["mini-grep", "Rust", "sample.txt"])
            .expect("valid arguments must parse");

        let config = Config::try_from(args).expect("valid arguments must produce configuration");

        assert_eq!(
            config.sources,
            vec![PatternSource::Argument(String::from("Rust"))]
        );
        assert!(matches!(config.pattern_options.mode, PatternMode::Regex));
        assert!(matches!(config.pattern_options.case, CaseMode::Sensitive));
        assert!(matches!(
            config.pattern_options.selection,
            Selection::Matching
        ))
    }

    #[test]
    fn preserves_search_and_output_flags() {
        let args = Args::try_parse_from([
            "mini-grep",
            "--ignore-case",
            "--fixed-strings",
            "--line-number",
            "--invert-match",
            "Rust",
            "sample.txt",
        ])
        .expect("compatible flags must parse");

        let config = Config::try_from(args).expect("compatible flags must produce configuration");

        assert!(matches!(config.pattern_options.mode, PatternMode::Literal));
        assert!(matches!(config.pattern_options.case, CaseMode::Insensitive));
        assert!(matches!(
            config.pattern_options.selection,
            Selection::NonMatching
        ));
    }

    #[test]
    fn rejects_an_empty_path() {
        let mut args = Args::try_parse_from(["mini-grep", "Rust", "sample.txt"])
            .expect("valid arguments must parse");

        args.inputs[1] = OsString::new();

        let error = Config::try_from(args).expect_err("configuration must reject an empty path");

        assert!(matches!(
            error,
            AppError::Usage("file path must not be empty")
        ));
    }

    #[test]
    fn preserves_an_empty_pattern() {
        let args = Args::try_parse_from(["mini-grep", "", "sample.txt"])
            .expect("an empty pattern must parse");

        let config = Config::try_from(args).expect("an empty pattern is valid");

        assert_eq!(config.sources, vec![PatternSource::Argument(String::new())])
    }

    fn positionals(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn resolves_a_positional_pattern_and_path() {
        let inputs = resolve_inputs(positionals(&["Rust", "sample.txt"]), vec![], vec![])
            .expect("positional search inputs must resolve");

        assert_eq!(
            inputs.sources,
            vec![PatternSource::Argument(String::from("Rust"))]
        );
        assert_eq!(inputs.path, PathBuf::from("sample.txt"));
    }

    #[test]
    fn explicit_sources_require_only_a_path() {
        let inputs = resolve_inputs(
            positionals(&["sample.txt"]),
            vec![String::from("Rust"), String::from("Go")],
            vec![PathBuf::from("patterns.txt")],
        )
        .expect("explicit sources must resolve");

        assert_eq!(
            inputs.sources,
            vec![
                PatternSource::Argument(String::from("Rust")),
                PatternSource::Argument(String::from("Go")),
                PatternSource::File(PathBuf::from("patterns.txt")),
            ]
        );
        assert_eq!(inputs.path, PathBuf::from("sample.txt"));
    }

    #[test]
    fn pattern_files_alone_supply_the_patterns() {
        let inputs = resolve_inputs(
            positionals(&["naïve file.txt"]),
            vec![],
            vec![
                PathBuf::from("first.patterns"),
                PathBuf::from("second.patterns"),
            ],
        )
        .expect("pattern-file sources must resolve without opening files");

        assert_eq!(
            inputs.sources,
            vec![
                PatternSource::File(PathBuf::from("first.patterns")),
                PatternSource::File(PathBuf::from("second.patterns")),
            ]
        );
        assert_eq!(inputs.path, PathBuf::from("naïve file.txt"));
    }

    #[test]
    fn empty_explicit_pattern_counts_as_a_source() {
        let inputs = resolve_inputs(positionals(&["sample.txt"]), vec![String::new()], vec![])
            .expect("an empty explicit pattern is valid");

        assert_eq!(inputs.sources, vec![PatternSource::Argument(String::new())]);
    }

    #[test]
    fn rejects_incorrect_positional_counts() {
        let cases: &[(&[&str], bool)] = &[
            (&[], false),
            (&["Rust"], false),
            (&["Rust", "sample.txt", "extra.txt"], false),
            (&[], true),
            (&["Rust", "sample.txt"], true),
        ];

        for &(values, explicit) in cases {
            let patterns = if explicit {
                vec![String::from("Rust")]
            } else {
                vec![]
            };

            assert!(
                resolve_inputs(positionals(values), patterns, vec![]).is_err(),
                "positionals: {values:?}, explicit sources: {explicit}"
            );
        }
    }

    #[test]
    fn rejects_empty_paths_and_unsupported_pattern_stdin() {
        assert!(resolve_inputs(positionals(&["Rust", ""]), vec![], vec![],).is_err());

        assert!(resolve_inputs(positionals(&[""]), vec![String::from("Rust")], vec![],).is_err());

        for path in ["", "-"] {
            assert!(
                resolve_inputs(
                    positionals(&["sample.txt"]),
                    vec![],
                    vec![PathBuf::from(path)],
                )
                .is_err()
            );
        }
    }

    #[test]
    fn context_counts_default_to_zero() {
        let args = Args::try_parse_from(["mini-grep", "Rust", "sample.txt"])
            .expect("valid arguments must parse");
        let config = Config::try_from(args).expect("valid arguments must produce configuration");

        assert_eq!(config.before_context, 0);
        assert_eq!(config.after_context, 0);
    }

    #[test]
    fn explicit_context_sides_override_combined_context_in_either_order() {
        let cases: &[(&[&str], usize, usize)] = &[
            (&["-C", "3"], 3, 3),
            (&["--context", "3"], 3, 3),
            (&["-A", "2"], 0, 2),
            (&["-B", "2"], 2, 0),
            (&["-C", "3", "-A", "1"], 3, 1),
            (&["-A", "1", "-C", "3"], 3, 1),
            (&["-C", "3", "-B", "0"], 0, 3),
            (&["-B", "0", "-C", "3"], 0, 3),
            (
                &[
                    "--context",
                    "3",
                    "--before-context",
                    "1",
                    "--after-context",
                    "0",
                ],
                1,
                0,
            ),
        ];

        for &(flags, before, after) in cases {
            let mut arguments = vec!["mini-grep"];
            arguments.extend_from_slice(flags);
            arguments.extend_from_slice(&["Rust", "sample.txt"]);

            let args = Args::try_parse_from(arguments).expect("context arguments must parse");
            let config =
                Config::try_from(args).expect("context arguments must produce configuration");

            assert_eq!(config.before_context, before, "flags: {flags:?}");
            assert_eq!(config.after_context, after, "flags: {flags:?}");
        }
    }

    #[test]
    fn context_counts_reject_negative_and_non_integer_values() {
        for flag in [
            "-A",
            "-B",
            "-C",
            "--after-context",
            "--before-context",
            "--context",
        ] {
            for value in ["-1", "abc", "1.5"] {
                assert!(
                    Args::try_parse_from(["mini-grep", flag, value, "Rust", "sample.txt",])
                        .is_err(),
                    "flag: {flag}, value: {value}"
                );
            }
        }
    }

    #[test]
    fn selection_limits_default_to_unlimited() {
        let args = Args::try_parse_from(["mini-grep", "Rust", "sample.txt"])
            .expect("valid arguments must parse");
        let config = Config::try_from(args).expect("valid arguments must produce configuration");

        assert_eq!(config.max_count, None);
        assert_eq!(config.max_results, None);
    }

    #[test]
    fn preserves_per_file_and_global_selection_limits() {
        let cases: &[(&[&str], Option<usize>, Option<usize>)] = &[
            (&["-m", "2"], Some(2), None),
            (&["--max-count", "2"], Some(2), None),
            (&["--max-results", "3"], None, Some(3)),
            (&["-m", "2", "--max-results", "3"], Some(2), Some(3)),
            (&["--max-results", "3", "-m", "2"], Some(2), Some(3)),
            (&["-m", "0", "--max-results", "0"], Some(0), Some(0)),
        ];

        for &(flags, per_file, global) in cases {
            let mut arguments = vec!["mini-grep"];
            arguments.extend_from_slice(flags);
            arguments.extend_from_slice(&["Rust", "sample.txt"]);

            let args = Args::try_parse_from(arguments).expect("limit arguments must parse");
            let config =
                Config::try_from(args).expect("limit arguments must produce configuration");

            assert_eq!(config.max_count, per_file, "flags: {flags:?}");
            assert_eq!(config.max_results, global, "flags: {flags:?}");
        }
    }

    #[test]
    fn selection_limits_reject_negative_and_non_integer_values() {
        for flag in ["-m", "--max-count", "--max-results"] {
            for value in ["-1", "abc", "1.5"] {
                assert!(
                    Args::try_parse_from(["mini-grep", flag, value, "Rust", "sample.txt",])
                        .is_err(),
                    "flag: {flag}, value: {value}"
                );
            }
        }
    }

    #[test]
    fn quiet_mode_defaults_to_disabled() {
        let args = Args::try_parse_from(["mini-grep", "Rust", "sample.txt"])
            .expect("valid arguments must parse");
        let config = Config::try_from(args).expect("valid arguments must produce configuration");

        assert!(!config.quiet);
    }

    #[test]
    fn quiet_mode_accepts_short_long_and_combined_flags() {
        for flag in ["-q", "--quiet", "-iFnq"] {
            let args = Args::try_parse_from(["mini-grep", flag, "Rust", "sample.txt"])
                .expect("quiet arguments must parse");
            let config =
                Config::try_from(args).expect("quiet arguments must produce configuration");

            assert!(config.quiet, "flag: {flag}");
        }
    }

    #[test]
    fn output_mode_defaults_to_lines() {
        let args = Args::try_parse_from(["mini-grep", "Rust", "sample.txt"])
            .expect("valid arguments must parse");
        let config = Config::try_from(args).expect("valid arguments must produce configuration");

        assert_eq!(config.output_mode, OutputMode::Lines);
    }

    #[test]
    fn resolves_file_list_output_modes() {
        let cases = [
            ("-l", OutputMode::FilesWithMatches),
            ("--files-with-matches", OutputMode::FilesWithMatches),
            ("--files-without-match", OutputMode::FilesWithoutMatch),
        ];

        for (flag, expected) in cases {
            let args = Args::try_parse_from([
                "mini-grep",
                "-q",
                "-n",
                "-C",
                "1",
                flag,
                "Rust",
                "sample.txt",
            ])
            .expect("file-list arguments must parse");
            let config =
                Config::try_from(args).expect("file-list arguments must produce configuration");

            assert_eq!(config.output_mode, expected, "flag: {flag}");
            assert!(config.quiet);
        }
    }

    #[test]
    fn file_list_modes_conflict_in_either_order() {
        for flags in [
            ["-l", "--files-without-match"],
            ["--files-without-match", "-l"],
            ["--files-with-matches", "--files-without-match"],
            ["--files-without-match", "--files-with-matches"],
        ] {
            let error =
                Args::try_parse_from(["mini-grep", flags[0], flags[1], "Rust", "sample.txt"])
                    .expect_err("file-list modes must conflict");

            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }

    #[test]
    fn resolves_count_output_modes() {
        for (flag, expected) in [
            ("-c", OutputMode::CountLines),
            ("--count", OutputMode::CountLines),
            ("--count-matches", OutputMode::CountMatches),
        ] {
            let args = Args::try_parse_from(["mini-grep", flag, "Rust", "sample.txt"])
                .expect("count arguments must parse");
            let config =
                Config::try_from(args).expect("count arguments must produce configuration");

            assert_eq!(config.output_mode, expected, "flag: {flag}");
        }
    }

    #[test]
    fn count_modes_reject_conflicting_flags_in_either_order() {
        let pairs = [
            ["-c", "--count-matches"],
            ["-c", "-l"],
            ["-c", "--files-without-match"],
            ["--count-matches", "-l"],
            ["--count-matches", "--files-without-match"],
            ["--count-matches", "-v"],
        ];

        for [first, second] in pairs {
            for flags in [[first, second], [second, first]] {
                let error =
                    Args::try_parse_from(["mini-grep", flags[0], flags[1], "Rust", "sample.txt"])
                        .expect_err("conflicting count arguments must fail");

                assert_eq!(
                    error.kind(),
                    clap::error::ErrorKind::ArgumentConflict,
                    "flags: {flags:?}"
                );
            }
        }
    }

    #[test]
    fn selected_line_counts_accept_inversion_and_quiet_mode() {
        let args = Args::try_parse_from(["mini-grep", "-cvq", "Rust", "sample.txt"])
            .expect("line counting supports inversion and quiet mode");
        let config = Config::try_from(args).expect("compatible count settings must validate");

        assert_eq!(config.output_mode, OutputMode::CountLines);
        assert!(config.quiet);
        assert!(matches!(
            config.pattern_options.selection,
            Selection::NonMatching
        ));
    }

    #[test]
    fn resolves_match_only_output_with_compatible_flags() {
        for flag in ["-o", "--only-matching", "-ionq"] {
            let args = Args::try_parse_from(["mini-grep", flag, "-C", "2", "Rust", "sample.txt"])
                .expect("match-only arguments must parse");
            let config =
                Config::try_from(args).expect("compatible match-only settings must validate");

            assert_eq!(config.output_mode, OutputMode::OnlyMatching, "flag: {flag}");
        }
    }

    #[test]
    fn match_only_rejects_conflicting_flags_in_either_order() {
        for mode in ["-o", "--only-matching"] {
            for conflict in ["-l", "--files-without-match", "-c", "--count-matches", "-v"] {
                for flags in [[mode, conflict], [conflict, mode]] {
                    let error = Args::try_parse_from([
                        "mini-grep",
                        flags[0],
                        flags[1],
                        "Rust",
                        "sample.txt",
                    ])
                    .expect_err("conflicting match-only arguments must fail");

                    assert_eq!(
                        error.kind(),
                        clap::error::ErrorKind::ArgumentConflict,
                        "flags: {flags:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn resolves_filename_visibility_without_changing_the_default() {
        let cases: &[(&[&str], bool)] = &[
            (&[], true),
            (&["-H"], true),
            (&["--with-filename"], true),
            (&["-h"], false),
            (&["--no-filename"], false),
        ];

        for &(flags, expected) in cases {
            let mut arguments = vec!["mini-grep"];
            arguments.extend_from_slice(flags);
            arguments.extend_from_slice(&["Rust", "sample.txt"]);

            let args = Args::try_parse_from(arguments).expect("filename arguments must parse");
            let config = Config::try_from(args).expect("filename arguments must validate");

            assert_eq!(config.show_filename, expected, "flags: {flags:?}");
        }
    }

    #[test]
    fn filename_flags_conflict_in_either_order() {
        for flags in [
            ["-H", "-h"],
            ["-h", "-H"],
            ["--with-filename", "--no-filename"],
            ["--no-filename", "--with-filename"],
        ] {
            let error =
                Args::try_parse_from(["mini-grep", flags[0], flags[1], "Rust", "sample.txt"])
                    .expect_err("filename flags must conflict");

            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }

    #[test]
    fn column_output_implies_line_numbers() {
        let cases: &[(&[&str], bool, bool, bool)] = &[
            (&[], false, false, true),
            (&["-n"], false, true, true),
            (&["--column"], true, true, true),
            (&["--column", "-n"], true, true, true),
            (&["-n", "--column"], true, true, true),
            (&["--column", "-h"], true, true, false),
        ];

        for &(flags, column, line_numbers, show_filename) in cases {
            let mut arguments = vec!["mini-grep"];
            arguments.extend_from_slice(flags);
            arguments.extend_from_slice(&["Rust", "sample.txt"]);

            let args = Args::try_parse_from(arguments).expect("column arguments must parse");
            let config = Config::try_from(args).expect("column arguments must validate");

            assert_eq!(config.column, column, "flags: {flags:?}");
            assert_eq!(config.line_numbers, line_numbers, "flags: {flags:?}");
            assert_eq!(config.show_filename, show_filename, "flags: {flags:?}");
        }
    }

    #[test]
    fn resolves_heading_settings_with_compatible_flags() {
        let cases: &[(&[&str], bool)] = &[
            (&[], false),
            (&["--heading"], true),
            (&["--heading", "-H"], true),
            (&["-H", "--heading"], true),
            (&["--heading", "--column"], true),
            (&["--heading", "-o"], true),
            (&["--heading", "-q"], true),
            (&["--heading", "-c"], true),
            (&["--heading", "-l"], true),
        ];

        for &(flags, expected) in cases {
            let mut arguments = vec!["mini-grep"];
            arguments.extend_from_slice(flags);
            arguments.extend_from_slice(&["Rust", "sample.txt"]);

            let args =
                Args::try_parse_from(arguments).expect("compatible heading arguments must parse");
            let config =
                Config::try_from(args).expect("compatible heading arguments must validate");

            assert_eq!(config.heading, expected, "flags: {flags:?}");
        }
    }

    #[test]
    fn headings_conflict_with_filename_suppression_in_either_order() {
        for flags in [
            ["--heading", "-h"],
            ["-h", "--heading"],
            ["--heading", "--no-filename"],
            ["--no-filename", "--heading"],
        ] {
            let error =
                Args::try_parse_from(["mini-grep", flags[0], flags[1], "Rust", "sample.txt"])
                    .expect_err("headings require filename visibility");

            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }

    #[test]
    fn resolves_null_filename_settings() {
        let cases: &[(&[&str], bool)] = &[
            (&[], false),
            (&["-0"], true),
            (&["--null"], true),
            (&["-l0"], true),
            (&["-0h"], true),
            (&["--null", "--column"], true),
            (&["--null", "-c"], true),
            (&["--null", "-q"], true),
        ];

        for &(flags, expected) in cases {
            let mut arguments = vec!["mini-grep"];
            arguments.extend_from_slice(flags);
            arguments.extend_from_slice(&["Rust", "sample.txt"]);

            let args =
                Args::try_parse_from(arguments).expect("compatible NUL arguments must parse");
            let config = Config::try_from(args).expect("compatible NUL arguments must validate");

            assert_eq!(config.null, expected, "flags: {flags:?}");
        }
    }

    #[test]
    fn null_filenames_conflict_with_headings_in_either_order() {
        for flags in [
            ["-0", "--heading"],
            ["--heading", "-0"],
            ["--null", "--heading"],
            ["--heading", "--null"],
        ] {
            let error =
                Args::try_parse_from(["mini-grep", flags[0], flags[1], "Rust", "sample.txt"])
                    .expect_err("NUL filenames and headings must conflict");

            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }
}
