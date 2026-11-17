use std::io::{self, StdinLock};
use std::path::PathBuf;

pub(crate) const LABEL: &str = "(standard input)";

pub(crate) fn lock() -> StdinLock<'static> {
    io::stdin().lock()
}

pub(crate) fn resolve_target(explicit: Option<PathBuf>, stdin_is_terminal: bool) -> PathBuf {
    explicit.unwrap_or_else(|| {
        if stdin_is_terminal {
            PathBuf::from(".")
        } else {
            PathBuf::from("-")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_target_uses_terminal_status() {
        assert_eq!(resolve_target(None, true), PathBuf::from("."));
        assert_eq!(resolve_target(None, false), PathBuf::from("-"));
    }

    #[test]
    fn explicit_targets_are_preserved_regardless_of_terminal_status() {
        for name in ["sample.txt", "folder", "-", "./-"] {
            for stdin_is_terminal in [false, true] {
                let expected = PathBuf::from(name);
                let actual = resolve_target(Some(expected.clone()), stdin_is_terminal);

                assert_eq!(
                    actual.as_os_str(),
                    expected.as_os_str(),
                    "target: {name}, terminal: {stdin_is_terminal}"
                );
            }
        }
    }
}
