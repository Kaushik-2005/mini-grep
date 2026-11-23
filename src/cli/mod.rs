mod args;
mod config;

pub(crate) mod commands;
pub(crate) mod completions;

use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches};

use crate::AppError;
use crate::config::file::{self as config_file, ColorSetting, FileConfig, SortSetting};

pub(crate) use config::{PathListSource, PatternSource};

pub(crate) enum Invocation {
    Search(Box<config::Config>),
    Config(commands::ConfigCommand),
    Completions(commands::CompletionShell),
}

pub(crate) fn parse() -> Result<Invocation, AppError> {
    let arguments: Vec<_> = std::env::args_os().collect();

    if let Some(command) = commands::parse(&arguments) {
        return command
            .map(Invocation::Config)
            .map_err(|error| AppError::Cli(Box::new(error)));
    }

    if let Some(shell) = commands::parse_completions(&arguments) {
        return shell
            .map(Invocation::Completions)
            .map_err(|error| AppError::Cli(Box::new(error)));
    }

    let matches = args::Args::command()
        .try_get_matches_from(arguments)
        .map_err(|error| AppError::Cli(Box::new(error)))?;

    let mut args =
        args::Args::from_arg_matches(&matches).map_err(|error| AppError::Cli(Box::new(error)))?;

    if !args.no_config {
        let file_config = config_file::load(std::path::Path::new(config_file::FILE_NAME))?;

        apply_file_defaults(&mut args, &matches, &file_config);
    }

    config::Config::try_from(args)
        .map(|config| Invocation::Search(Box::new(config)))
        .map_err(|error| match error {
            AppError::Usage(message) => AppError::Cli(Box::new(
                args::Args::command().error(clap::error::ErrorKind::ValueValidation, message),
            )),
            other => other,
        })
}

fn apply_file_defaults(args: &mut args::Args, matches: &clap::ArgMatches, config: &FileConfig) {
    if matches.value_source("threads") != Some(ValueSource::CommandLine)
        && let Some(threads) = config.threads
    {
        args.threads = threads;
    }

    if matches.value_source("color") != Some(ValueSource::CommandLine)
        && let Some(color) = config.color
    {
        args.color = match color {
            ColorSetting::Auto => args::ColorArg::Auto,
            ColorSetting::Always => args::ColorArg::Always,
            ColorSetting::Never => args::ColorArg::Never,
        };
    }

    if matches.value_source("sort") != Some(ValueSource::CommandLine)
        && let Some(sort) = config.sort
    {
        args.sort = match sort {
            SortSetting::None => args::SortArg::None,
            SortSetting::Path => args::SortArg::Path,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::num::NonZeroUsize;

    use clap::FromArgMatches;

    fn resolve(arguments: &[&str], config: &FileConfig) -> args::Args {
        let matches = args::Args::command()
            .try_get_matches_from(arguments)
            .expect("valid arguments must parse");

        let mut args =
            args::Args::from_arg_matches(&matches).expect("parsed matches must produce arguments");

        apply_file_defaults(&mut args, &matches, config);
        args
    }

    fn custom_defaults() -> FileConfig {
        FileConfig {
            threads: NonZeroUsize::new(4),
            color: Some(ColorSetting::Never),
            sort: Some(SortSetting::Path),
        }
    }

    #[test]
    fn unspecified_file_settings_preserve_builtin_defaults() {
        let args = resolve(&["mini-grep", "Rust", "sample.txt"], &FileConfig::default());

        assert_eq!(args.threads.get(), 1);
        assert_eq!(args.color, args::ColorArg::Auto);
        assert_eq!(args.sort, args::SortArg::None);
    }

    #[test]
    fn file_settings_replace_implicit_cli_defaults() {
        let args = resolve(&["mini-grep", "Rust", "sample.txt"], &custom_defaults());

        assert_eq!(args.threads.get(), 4);
        assert_eq!(args.color, args::ColorArg::Never);
        assert_eq!(args.sort, args::SortArg::Path);
    }

    #[test]
    fn explicit_builtin_values_override_file_settings() {
        let args = resolve(
            &[
                "mini-grep",
                "-j1",
                "--color=auto",
                "--sort=none",
                "Rust",
                "sample.txt",
            ],
            &custom_defaults(),
        );

        assert_eq!(args.threads.get(), 1);
        assert_eq!(args.color, args::ColorArg::Auto);
        assert_eq!(args.sort, args::SortArg::None);
    }

    #[test]
    fn explicit_cli_values_override_only_their_own_settings() {
        let args = resolve(
            &["mini-grep", "-j", "2", "Rust", "sample.txt"],
            &custom_defaults(),
        );

        assert_eq!(args.threads.get(), 2);
        assert_eq!(args.color, args::ColorArg::Never);
        assert_eq!(args.sort, args::SortArg::Path);
    }

    #[test]
    fn accepts_no_config_for_search_and_file_enumeration() {
        for arguments in [
            vec!["mini-grep", "--no-config", "Rust", "sample.txt"],
            vec!["mini-grep", "--no-config", "--files", "."],
        ] {
            let matches = args::Args::command()
                .try_get_matches_from(arguments)
                .expect("--no-config arguments must parse");

            let args = args::Args::from_arg_matches(&matches)
                .expect("parsed matches must produce arguments");

            assert!(args.no_config);
        }
    }
}
