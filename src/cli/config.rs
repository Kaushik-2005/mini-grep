use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::args::Args;
use crate::AppError;
use crate::discovery::DiscoveryOptions;
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
            line_numbers: args.line_numbers,
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
}
