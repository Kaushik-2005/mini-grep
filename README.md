# mini-grep

## Project status

Milestones 1 and 2 are complete. Single-file search supports buffered input, contextual errors, and exit codes. Milestone 3 adds dedicated CLI parsing and flags.

The developer confirmed that all 31 Windows tests, `cargo fmt`, and `cargo clippy --all-targets --all-features` passed. The developer also confirmed the file-search output. The AI reviewed the source but did not run the checks.

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
cargo run -- Rust tests/fixtures/text/demo.txt
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

The executable requires exactly two positional arguments: `mini-grep <PATTERN> <PATH>`. It searches one UTF-8 file with buffered, line-by-line input. It writes `path:line:text` to stdout and diagnostics to stderr. Paths remain native operating-system paths; displayed paths can replace invalid Unicode with replacement characters.

| Exit status | Meaning |
|---|---|
| `0` | At least one matching line |
| `1` | No matching lines |
| `2` | Usage, open, read, or output error |

On a read error, search stops. Previously printed matches remain on stdout and the final status is `2`. Invalid UTF-8 content is a read error. A non-UTF-8 pattern is a usage error.

## Examples

For pattern `Rust` and input `first\nRust is useful`, the library returns one result: line 2 with text `Rust is useful`.

## Design summary

- `src/lib.rs` exposes the search module.
- `src/main.rs` calls `run`, reports errors, and selects the exit status.
- `src/app.rs` validates positional arguments and connects input, matching, and output.
- `src/error.rs` preserves the operation, path, and underlying I/O error.
- `src/input/reader.rs` opens files and yields numbered, owned lines.
- `src/search/mod.rs` exposes the matcher and result type.
- `src/search/literal.rs` searches in-memory UTF-8 text.
- `src/search/result.rs` defines `LineMatch`.
- `src/output/terminal.rs` writes `path:line:text` records and propagates write errors. The application flushes stdout.
- `tests/search.rs` tests the public library interface without filesystem input.
- `tests/app.rs` checks executable output, stderr, and successful exit status.
- `tests/cli.rs` checks CLI edge cases and error exit statuses.
- `tests/fixtures/text/demo.txt` supplies a file-search fixture.

The search core performs no input or output. The caller owns the input and must keep it alive while using borrowed results.

## Tests

Tests cover matching lines, absent patterns, case sensitivity, empty input, empty patterns, repeated occurrences, Unicode, and CRLF line endings.

CLI tests cover paths with spaces, usage errors, no matches, empty files and lines, missing files, directory input, invalid UTF-8, and partial output before a read error. A Windows-only test checks opening a file blocked by an exclusive handle. Unit tests cover simulated read and write failures and error sources. Unix-only tests for non-UTF-8 paths and patterns are present but have not been verified on Unix.

Terminal command (run from repository root)
```powershell
cargo fmt
cargo clippy --all-targets --all-features
cargo test --all-features
```

## Limitations

The in-memory `search` API collects results in a `Vec`. The CLI processes one line at a time; a very long line can still require substantial memory. Search accepts UTF-8 content only. Flags, help/version, multiple files, directory traversal, stdin, regex, and byte offsets are not implemented. Arguments are currently positional, including values that begin with a hyphen. Broken pipes currently produce an output error and exit status `2`; pipeline-specific handling is planned for Milestone 7.

## Planned features

Follow `roadmap.md` for milestone order: dedicated CLI parsing and flags, recursive discovery, regex, context, pipelines, parallel search, benchmarks, JSON, configuration, and releases.

TUI and local indexing are optional extensions after the production CLI.

## Development workflow

The human writes and runs implementation code, tests, configuration, and scripts. The AI teaches, reviews, supplies requested code, and maintains this README. Follow `AGENTS.md` and complete each milestone's acceptance criteria before advancing.
