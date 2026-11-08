# mini-grep

## Project status

Milestones 1 through 3 are complete. Single-file search includes clap parsing, validated configuration, case selection, inversion, and optional line numbers. Milestone 4 adds recursive discovery and filtering.

The developer reported that all 63 Windows tests passed and confirmed completion of the final formatting and Clippy checks. The AI reviewed the source but did not run the checks. Unix-only tests remain unverified on Unix.

## Overview

mini-grep is a standalone Rust command-line tool for local text search and a Rust learning project.

## Supported platforms

Development is on Windows. Linux and macOS are planned targets. Cross-platform checks are not yet verified.

## Installation

Use a stable Rust toolchain with Cargo. Release binaries are not available yet.

## Quick start

Run from the repository root.

Terminal command (run from repository root)
```powershell
cargo run -- -n Rust tests/fixtures/text/demo.txt
```

The executable opens the specified file at runtime and searches for `Rust`.

Expected output (illustrative)
```text
tests/fixtures/text/demo.txt:2:Rust makes ownership explicit.
tests/fixtures/text/demo.txt:4:Rust rewards careful design.
```

## Implemented behavior and commands

The library exposes `mini_grep::search::search(pattern, text)` and `mini_grep::search::LineMatch`.

- Search is case-sensitive and uses literal substring matching.
- Each matching line produces one result with a 1-based line number.
- Results borrow line text from the input. They exclude LF and CRLF line endings.
- Repeated occurrences on one line produce one result.
- An empty pattern matches every line returned by `str::lines()`.
- Empty input returns no results.

The search command is `mini-grep [OPTIONS] <PATTERN> <PATH>`. It searches one UTF-8 file with buffered, line-by-line input. Default output is `path:text`; `-n` adds the 1-based line number as `path:line:text`. Diagnostics go to stderr. Paths remain native operating-system paths; displayed paths can replace invalid Unicode with replacement characters.

| Option | Behavior |
|---|---|
| `-i`, `--ignore-case` | Compare Unicode-lowercased pattern and line |
| `-F`, `--fixed-strings` | Select literal matching, which is currently the default |
| `-n`, `--line-number` | Include line numbers |
| `-v`, `--invert-match` | Select lines that do not match |
| `--help` | Print help to stdout and exit `0` |
| `-V`, `--version` | Print version to stdout and exit `0` |

Compatible search flags can be combined, such as `-in`. Repeated boolean flags such as `-ii` are rejected. `-h` is reserved and currently rejected. Use `--` before a pattern that begins with a hyphen. A search requires both a pattern and an explicit file path.

Case-insensitive matching uses lowercasing, not full Unicode case folding or normalization. For example, `CAFÉ` matches `café`, but `STRASSE` does not match `Straße`. Output preserves original line text. Inversion applies after case handling; an inverted empty pattern selects no lines.

| Exit status | Meaning |
|---|---|
| `0` | At least one selected line, or successful help/version output |
| `1` | No selected lines |
| `2` | Usage, open, read, or output error |

On a read error, search stops. Previously printed matches remain on stdout and the final status is `2`. Invalid UTF-8 content is a read error. A non-UTF-8 pattern is a usage error.

## Examples

For pattern `Rust` and input `first\nRust is useful`, the library returns one result: line 2 with text `Rust is useful`.

Run a case-insensitive search with line numbers.

Terminal command (run from repository root)
```powershell
cargo run -- -in rust tests/fixtures/text/demo.txt
```

Show command help.

Terminal command (run from repository root)
```powershell
cargo run -- --help
```

## Design summary

- `src/lib.rs` exposes the search module.
- `src/main.rs` calls `run`, reports errors, and selects the exit status.
- `src/cli/args.rs` defines clap syntax and help text.
- `src/cli/config.rs` converts parsed arguments into validated settings.
- `src/app.rs` connects configuration, input, matching, and output.
- `src/error.rs` preserves the operation, path, and underlying I/O error.
- `src/input/reader.rs` opens files and yields numbered, owned lines.
- `src/search/mod.rs` exposes the matcher and result type.
- `src/search/literal.rs` provides the case-sensitive in-memory search API and an internal reusable matcher with case handling and inversion.
- `src/search/result.rs` defines `LineMatch`.
- `src/output/terminal.rs` writes records with optional line numbers and propagates write errors. The application flushes stdout.
- `tests/search.rs` tests the public library interface without filesystem input.
- `tests/app.rs` checks executable output, stderr, and successful exit status.
- `tests/cli.rs` checks CLI edge cases and error exit statuses.
- `tests/fixtures/text/demo.txt` supplies a file-search fixture.

The search core performs no input or output. The caller owns the input and must keep it alive while using borrowed results.

## Tests

Tests cover matching lines, absent patterns, case sensitivity, empty input, empty patterns, repeated occurrences, Unicode, and CRLF line endings.

CLI tests cover paths with spaces, usage errors, no matches, empty files and lines, missing files, directory input, invalid UTF-8, and partial output before a read error. A Windows-only test checks opening a file blocked by an exclusive handle. Unit tests cover simulated read and write failures and error sources. Unix-only tests for non-UTF-8 paths and patterns are present but have not been verified on Unix.

Parser, configuration, matcher, and executable tests cover defaults, short and long flags, combined and repeated flags, inversion, lowercasing limits, the `--` separator, help/version, and empty paths.

Terminal command (run from repository root)
```powershell
cargo fmt
cargo clippy --all-targets --all-features
cargo test --all-features
```

## Limitations

The in-memory `search` API collects results in a `Vec`. The CLI processes one line at a time; a very long line can still require substantial memory. Case-insensitive matching allocates a lowercase copy of each line and prepares the lowercase pattern once. Search accepts UTF-8 content only. Multiple files, directory traversal, stdin, regex, and byte offsets are not implemented. Broken pipes currently produce an output error and exit status `2`; pipeline-specific handling is planned for Milestone 7.

## Planned features

Follow `roadmap.md` for milestone order: recursive discovery, regex, context, pipelines, parallel search, benchmarks, JSON, configuration files, and releases.

The CLI uses clap 4 with its `derive` feature to define arguments, generate help/version output, and report syntax errors. Clap types stay outside the search core.

TUI and local indexing are optional extensions after the production CLI.

## Development workflow

The human writes and runs implementation code, tests, configuration, and scripts. The AI teaches, reviews, supplies requested code, and maintains this README. Follow `AGENTS.md` and complete each milestone's acceptance criteria before advancing.
