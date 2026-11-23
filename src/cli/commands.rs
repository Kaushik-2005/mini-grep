use std::ffi::{OsStr, OsString};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "mini-grep config", about = "Inspect local configuration.")]
struct ConfigArgs {
    #[command(subcommand)]
    command: ConfigCommand,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub(crate) enum ConfigCommand {
    /// Print resolved configuration defaults as JSON
    Show {
        /// Ignore the local configuration file
        #[arg(long)]
        no_config: bool,
    },

    /// Report the local configuration file path
    Path,
}

/// Recognizes configuration commands from a complete process argument list.
///
/// Unrecognized prefixes remain ordinary search arguments.
pub(crate) fn parse(arguments: &[OsString]) -> Option<Result<ConfigCommand, clap::Error>> {
    if arguments.get(1).map(OsString::as_os_str) != Some(OsStr::new("config")) {
        return None;
    }

    let command_arguments = std::iter::once(OsString::from("mini-grep config"))
        .chain(arguments.iter().skip(2).cloned());

    Some(ConfigArgs::try_parse_from(command_arguments).map(|args| args.command))
}

/// Recognizes completion commands from q complete process argument list.
pub(crate) fn parse_completions(
    arguments: &[OsString],
) -> Option<Result<CompletionShell, clap::Error>> {
    if arguments.get(1).map(OsString::as_os_str) != Some(OsStr::new("completions")) {
        return None;
    }

    let command_arguments = std::iter::once(OsString::from("mini-grep completions"))
        .chain(arguments.iter().skip(2).cloned());

    Some(CompletionArgs::try_parse_from(command_arguments).map(|args| args.shell))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
}

#[derive(Debug, Parser)]
#[command(
    name = "mini-grep completions",
    about = "Print a shell completion script."
)]
struct CompletionArgs {
    /// Shell for which to generate completions
    #[arg(value_enum)]
    shell: CompletionShell,
}

pub(crate) fn config_command() -> clap::Command {
    ConfigArgs::command().name("config")
}

pub(crate) fn completion_command() -> clap::Command {
    CompletionArgs::command().name("completions")
}

#[cfg(test)]
mod tests {
    use super::*;

    use clap::error::ErrorKind;

    fn arguments(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_show_with_optional_loading_bypass() {
        for (values, expected) in [
            (
                vec!["mini-grep", "config", "show"],
                ConfigCommand::Show { no_config: false },
            ),
            (
                vec!["mini-grep", "config", "show", "--no-config"],
                ConfigCommand::Show { no_config: true },
            ),
        ] {
            let command = parse(&arguments(&values))
                .expect("config prefix must select utility parsing")
                .expect("valid utility command must parse");

            assert_eq!(command, expected);
        }
    }

    #[test]
    fn parses_path_without_search_arguments() {
        let command = parse(&arguments(&["mini-grep", "config", "path"]))
            .expect("config prefix must select utility parsing")
            .expect("path command must parse");

        assert_eq!(command, ConfigCommand::Path);
    }

    #[test]
    fn preserves_search_arguments_and_separator() {
        for values in [
            vec!["mini-grep", "Rust", "sample.txt"],
            vec!["mini-grep", "--", "config", "show"],
            vec!["mini-grep", "-F", "config", "sample.txt"],
            vec!["mini-grep"],
        ] {
            assert!(parse(&arguments(&values)).is_none());
        }
    }

    #[test]
    fn utility_help_does_not_require_search_arguments() {
        for values in [
            vec!["mini-grep", "config", "--help"],
            vec!["mini-grep", "config", "show", "--help"],
            vec!["mini-grep", "config", "path", "--help"],
        ] {
            let error = parse(&arguments(&values))
                .expect("config prefix must select utility parsing")
                .expect_err("help must return a display request");

            assert_eq!(error.kind(), ErrorKind::DisplayHelp);
            assert_eq!(error.exit_code(), 0);
        }
    }

    #[test]
    fn rejects_missing_or_unknown_utility_commands() {
        for values in [
            vec!["mini-grep", "config"],
            vec!["mini-grep", "config", "unknown"],
            vec!["mini-grep", "config", "show", "extra"],
            vec!["mini-grep", "config", "path", "--unknown"],
        ] {
            let error = parse(&arguments(&values))
                .expect("config prefix must select utility parsing")
                .expect_err("invalid utility command must fail");

            assert_eq!(error.exit_code(), 2);
        }
    }

    #[test]
    fn parses_supported_completion_shells() {
        for (name, expected) in [
            ("bash", CompletionShell::Bash),
            ("zsh", CompletionShell::Zsh),
            ("fish", CompletionShell::Fish),
            ("powershell", CompletionShell::PowerShell),
        ] {
            let shell = parse_completions(&arguments(&["mini-grep", "completions", name]))
                .expect("completions prefix must select utility parsing")
                .expect("supported shell must parse");

            assert_eq!(shell, expected);
        }
    }

    #[test]
    fn completion_help_does_not_require_a_shell() {
        let error = parse_completions(&arguments(&["mini-grep", "completions", "--help"]))
            .expect("completions prefix must select utility parsing")
            .expect_err("help must return a display request");

        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        assert_eq!(error.exit_code(), 0);
    }

    #[test]
    fn rejects_missing_unknown_or_extra_completion_arguments() {
        for values in [
            vec!["mini-grep", "completions"],
            vec!["mini-grep", "completions", "unknown"],
            vec!["mini-grep", "completions", "power-shell"],
            vec!["mini-grep", "completions", "bash", "extra"],
        ] {
            let error = parse_completions(&arguments(&values))
                .expect("completions prefix must select utility parsing")
                .expect_err("invalid completion arguments must fail");

            assert_eq!(error.exit_code(), 2);
        }
    }

    #[test]
    fn completion_parser_preserves_ordinary_search_arguments() {
        for values in [
            vec!["mini-grep", "--", "completions", "sample.txt"],
            vec!["mini-grep", "-F", "completions", "sample.txt"],
            vec!["mini-grep", "config", "show"],
            vec!["mini-grep"],
        ] {
            assert!(parse_completions(&arguments(&values)).is_none());
        }
    }
}
