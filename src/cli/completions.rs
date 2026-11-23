use clap::{Command, CommandFactory};
use clap_complete::aot::{Shell, generate as generate_completion};

use super::args::Args;
use super::commands::{self, CompletionShell};

/// Builds metadata for search arguments and utility commands.
///
/// Actual invocation routing remains in the CLI parser.
fn command() -> Command {
    Args::command()
        .subcommand(commands::config_command())
        .subcommand(commands::completion_command())
}

/// Generates a completion script without accessing files or stdout.
pub(crate) fn generate(shell: CompletionShell) -> Vec<u8> {
    let generator = match shell {
        CompletionShell::Bash => Shell::Bash,
        CompletionShell::Zsh => Shell::Zsh,
        CompletionShell::Fish => Shell::Fish,
        CompletionShell::PowerShell => Shell::PowerShell,
    };

    let mut command = command();
    let mut script = Vec::new();

    generate_completion(generator, &mut command, "mini-grep", &mut script);

    script
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_command_definition_is_valid() {
        command().debug_assert();
    }

    #[test]
    fn generates_scripts_with_search_flags_and_utility_commands() {
        for shell in [
            CompletionShell::Bash,
            CompletionShell::Zsh,
            CompletionShell::Fish,
            CompletionShell::PowerShell,
        ] {
            let bytes = generate(shell);
            let script = String::from_utf8(bytes).expect("generated script must be UTF-8");

            assert!(!script.is_empty(), "shell: {shell:?}");

            for expected in [
                "mini-grep",
                "json",
                "stats",
                "no-config",
                "config",
                "completions",
                "show",
                "path",
            ] {
                assert!(
                    script.contains(expected),
                    "shell {shell:?} omitted {expected}"
                );
            }
        }
    }
}
