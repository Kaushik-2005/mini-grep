# Changelog

## 0.1.0 — Unreleased

### Added

- Literal and regex search with case, word, line, and inversion options.
- Multiple patterns from arguments, files, and standard input.
- Recursive discovery with ignore rules, globs, file types, and size limits.
- Standard input search and target-path lists.
- Context lines, result limits, counts, filename output, and highlighting.
- JSON Lines output and search statistics.
- Parallel file search, path sorting, and cancellation.
- Local configuration with explicit CLI overrides.
- Configuration inspection and completion script generation for Bash,
  Zsh, Fish, and PowerShell.
- Property tests and CI for Windows, Linux, and macOS.
- Dual licensing under MIT OR Apache-2.0.

### Requirements

- Source builds require Rust 1.88 or newer.

### Limitations

- Search input must be valid UTF-8.
- Binary input is skipped by default. `--text` permits NUL bytes but
  still requires valid UTF-8.
- Interactive completion behavior has not been verified in every shell.