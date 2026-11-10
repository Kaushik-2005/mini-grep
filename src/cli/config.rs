use std::path::PathBuf;

use super::args::Args;
use crate::AppError;
use crate::discovery::DiscoveryOptions;

#[derive(Debug)]
pub(crate) struct Config {
    pub(crate) pattern: String,
    pub(crate) path: PathBuf,
    pub(crate) ignore_case: bool,
    pub(crate) line_numbers: bool,
    pub(crate) invert_match: bool,
    pub(crate) discovery: DiscoveryOptions,
}

impl TryFrom<Args> for Config {
    type Error = AppError;

    fn try_from(args: Args) -> Result<Self, Self::Error> {
        if args.path.as_os_str().is_empty() {
            return Err(AppError::Usage("file path must not be empty"));
        }

        Ok(Self {
            pattern: args.pattern,
            path: args.path,
            ignore_case: args.ignore_case,
            line_numbers: args.line_numbers,
            invert_match: args.invert_match,
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

        assert_eq!(config.pattern, "Rust");
        assert_eq!(config.path, PathBuf::from("sample.txt"));
        assert!(!config.ignore_case);
        assert!(!config.line_numbers);
        assert!(!config.invert_match);
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

        assert!(config.ignore_case);
        assert!(config.line_numbers);
        assert!(config.invert_match);
    }

    #[test]
    fn rejects_an_empty_path() {
        let mut args = Args::try_parse_from(["mini-grep", "Rust", "sample.txt"])
            .expect("valid arguments must parse");

        args.path = PathBuf::new();

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

        assert!(config.pattern.is_empty());
    }
}
