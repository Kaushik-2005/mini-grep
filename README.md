# mini-grep

## Project status

Milestone 1 is complete. The pure literal search core, application wiring, and in-memory executable demo are implemented. Milestone 2 adds runtime file input and the first search CLI.

The developer reported that all nine integration tests, `cargo fmt`, and `cargo clippy --all-targets --all-features` passed. The developer also confirmed the demo output. The AI reviewed the source but did not run these checks.

## Overview

mini-grep is a Rust learning project for local text search. The planned product is a standalone command-line tool.

## Supported platforms

Development is on Windows. Linux and macOS are planned targets. Cross-platform checks are not yet verified.

## Installation

Use a stable Rust toolchain with Cargo. Release binaries are not available yet.

## Quick start

Run from the repository root.

Terminal command (run from repository root)
```powershell
cargo run
```

The executable searches an embedded sample for `Rust`. It does not read a file at runtime.

Expected output (illustrative)
```text
2:Rust makes ownership explicit.
4:Rust rewards careful design.
```

## Implemented behavior and commands

The library exposes `mini_grep::search::search(pattern, text)` and `mini_grep::search::LineMatch`.

- Search is case-sensitive and uses literal substring matching.
- Each matching line produces one result with a 1-based line number.
- Results borrow line text from the input. They exclude LF and CRLF line endings.
- Repeated occurrences on one line produce one result.
- An empty pattern matches every line returned by `str::lines()`.
- Empty input returns no results.

The executable does not accept search arguments yet.

## Examples

For pattern `Rust` and input `first\nRust is useful`, the library returns one result: line 2 with text `Rust is useful`.

## Design summary

- `src/lib.rs` exposes the search module.
- `src/main.rs` calls the library's `run` function and returns output errors.
- `src/app.rs` connects the embedded sample, matcher, and terminal writer.
- `src/search/mod.rs` exposes the matcher and result type.
- `src/search/literal.rs` searches in-memory UTF-8 text.
- `src/search/result.rs` defines `LineMatch`.
- `src/output/terminal.rs` writes `line_number:text` records and propagates write and flush errors.
- `tests/search.rs` tests the public library interface without filesystem input.
- `tests/app.rs` checks executable output, stderr, and successful exit status.
- `tests/fixtures/text/demo.txt` supplies the sample embedded at compile time.

The search core performs no input or output. The caller owns the input and must keep it alive while using borrowed results.

## Tests

Tests cover matching lines, absent patterns, case sensitivity, empty input, empty patterns, repeated occurrences, Unicode, and CRLF line endings.

An executable integration test checks the full demo path. Output failure behavior is not yet covered by tests.

Terminal command (run from repository root)
```powershell
cargo fmt
cargo clippy --all-targets --all-features
cargo test --all-features
```

## Limitations

The search core accepts `&str`, which requires valid UTF-8. It collects results in a `Vec`. Runtime file reading, CLI search, byte offsets, and streaming execution are not implemented. The demo uses Rust's default error exit behavior; the planned 0/1/2 search exit policy and broken-pipe handling are not implemented yet.

## Planned features

Follow `roadmap.md` for milestone order: file input, CLI parsing, recursive discovery, regex, context, pipelines, parallel search, benchmarks, JSON, configuration, and releases.

TUI and local indexing are optional extensions after the production CLI.

## Development workflow

The human writes and runs implementation code, tests, configuration, and scripts. The AI teaches, reviews, supplies requested code, and maintains this README. Follow `AGENTS.md` and complete each milestone's acceptance criteria before advancing.
