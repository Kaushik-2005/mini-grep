# mini-grep

## Project status

Active development. The CLI supports local regex and literal search in files and directories, recursive traversal, path filters, ignore controls, symbolic links, and contextual error diagnostics.

The developer reports that all Windows tests pass, including regex modes and pattern-source validation. Unix-only tests remain unverified on Unix.

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

The library exposes `mini_grep::search::search(pattern, text)`, `mini_grep::search::LineMatch`, and `mini_grep::search::MatchSpan`.

`MatchSpan` validates zero-based, end-exclusive byte ranges against UTF-8 text. It accepts zero-width spans and rejects reversed ranges, out-of-bounds offsets, and offsets inside a Unicode character. Each `LineMatch` contains the original line text and its match spans. Inverted selected lines have an empty span list.

- Search is case-sensitive and uses literal substring matching.
- Each matching line produces one result with a 1-based line number.
- Results borrow line text from the input. They exclude LF and CRLF line endings.
- Repeated occurrences on one line produce one result.
- An empty pattern matches every line returned by `str::lines()`.
- Empty input returns no results.

The CLI uses regex matching by default. Use `mini-grep [OPTIONS] <PATTERN> <PATH>`, or supply patterns with `-e` and `-f` and give only `<PATH>` as the positional argument. Supply one file or directory. Directories are searched recursively for regular files, using buffered, line-by-line UTF-8 input. Default output is `path:text`; `-n` adds the 1-based line number as `path:line:text`. Line numbers restart for each file. Result order is unspecified. Diagnostics go to stderr. Paths remain native operating-system paths; displayed paths can replace invalid Unicode with replacement characters.

| Option | Behavior |
|---|---|
| `-i`, `--ignore-case` | Use Unicode-aware case-insensitive matching |
| `-F`, `--fixed-strings` | Select literal matching; the CLI default is regex |
| `-S`, `--smart-case` | Ignore case when no supplied pattern contains an uppercase character |
| `-w`, `--word-regexp` | Require non-word characters or line boundaries around a match |
| `-x`, `--line-regexp` | Require a match covering the entire line |
| `-e PATTERN`, `--regexp PATTERN` | Add a pattern; repeat to add more |
| `-f FILE`, `--file FILE` | Read one pattern per line from a UTF-8 file; repeat to add more files |
| `-n`, `--line-number` | Include line numbers |
| `-v`, `--invert-match` | Select lines that do not match |
| `--hidden` | Include hidden files and directories during discovery |
| `--no-ignore` | Disable all supported ignore-file rules and their read checks |
| `--no-ignore-vcs` | Disable Git ignore rules and their read checks; retain `.ignore` rules |
| `--max-depth DEPTH` | Limit traversal depth; the supplied root is depth `0` |
| `--max-filesize BYTES` | Search files whose size is at most this byte limit, including explicit roots |
| `-g GLOB`, `--glob GLOB` | Include matching paths; prefix with `!` to exclude; repeat to add patterns |
| `--type TYPE` | Include a built-in file type; repeat to include more types |
| `--type-not TYPE` | Exclude a built-in file type; repeat to exclude more types |
| `-L`, `--follow` | Follow symbolic links; report broken links and directory loops |
| `--help` | Print help to stdout and exit `0` |
| `-V`, `--version` | Print version to stdout and exit `0` |

Compatible search flags can be combined, such as `-in`. Repeated boolean flags such as `-ii` are rejected. `-i` conflicts with `-S`; `-w` conflicts with `-x`. `-h` is reserved and currently rejected. Use `--` before a positional pattern that begins with a hyphen, or use `--regexp=-needle`.

Patterns from all sources are combined with OR. A selected line is printed once. Inversion applies after combining patterns. Pattern files preserve spaces, hash characters, duplicates, and blank lines; LF and CRLF endings are removed. An empty file supplies no patterns and selects no lines. A blank line supplies an empty pattern and matches every line in the default scope. `-v` reverses these selections. `-f -` is rejected because stdin is not supported. To read a file named `-`, use a path such as `./-`.

Smart case checks the raw text of every supplied pattern, including regex syntax. Any uppercase character makes the default case setting sensitive for all patterns. Regex inline flags can override that setting within an expression. Whole-word boundaries use Unicode word characters, including letters, digits, combining marks, and underscores. Whole-line matching excludes the line ending. An empty pattern with `-x` matches only empty lines.

`--no-ignore` takes precedence over `--no-ignore-vcs` in either argument order. These switches do not enable hidden entries; add `--hidden` for that. Depth and size limits accept nonnegative integers only. Size suffixes such as `1KB` are not supported. A directory root with maximum depth `0` yields no candidate files. An explicit file root remains at depth `0` and still obeys the size limit.

Globs are case-sensitive, independently of `-i`. The last matching glob wins. With positive globs, files must match an inclusion; with only exclusions, other files remain eligible. Anchored globs are relative to the directory root, or the parent directory of an explicit file root. Explicit file roots obey globs. Positive globs can override ignore and hidden-file exclusions, but cannot restore traversal through an excluded directory. Depth and size limits still apply. Invalid globs fail before search output.

File types use the definitions bundled with `ignore`, such as `rust` and `toml`. Multiple inclusions form a union. Type exclusions win in either argument order. With no type inclusions, other types remain eligible. Type restrictions apply to explicit roots and cannot be bypassed by glob inclusions. Selecting a type does not itself include hidden or ignored files. Unknown type names fail before search output.

Directory searches skip symbolic links by default. An explicit symlink root requires `--follow`. When following links, output and path filters use the alias path, and size limits use the target file's metadata. Different aliases can search the same underlying file more than once. Broken links and directory loops produce diagnostics and final status `2`; other files continue.

CLI case-insensitive matching uses Unicode simple case folding without normalization. For example, `CAFÉ` matches `café`, but `STRASSE` does not match `Straße`. Output preserves original line text. In the default scope, an inverted empty pattern selects no lines.

Each pattern yields nonoverlapping matches. Spans from different patterns are sorted and identical ranges are removed; distinct overlaps remain. Offsets refer to bytes in the original line, and zero-width matches are retained. Highlighting and match-only output are not implemented yet.

| Exit status | Meaning |
|---|---|
| `0` | At least one selected line, or successful help/version output |
| `1` | No selected lines |
| `2` | Usage, pattern, discovery, open, read, or output error |

All pattern files are loaded and all patterns are compiled before target discovery. A pattern-file or compilation failure stops execution before search output. Discovery, target-file open, and target-file read errors are reported while other available candidates continue. A read error stops the affected file. Previously printed matches remain on stdout. Any operational error makes the final status `2`, even if selected lines were printed. Output errors stop execution. Invalid UTF-8 content is a read error. A non-UTF-8 pattern is a usage error.

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

Search text files recursively with line numbers.

Terminal command (run from repository root)
```powershell
cargo run -- -n -g "*.txt" Rust tests/fixtures/text
```

## Design summary

- `src/lib.rs` exposes the search module.
- `src/main.rs` calls `run`, reports errors, and selects the exit status.
- `src/cli/args.rs` defines clap syntax and help text.
- `src/cli/config.rs` converts parsed arguments into validated settings.
- `src/app.rs` connects configuration, input, matching, and output.
- `src/discovery/walker.rs` yields regular-file candidates and traversal errors without reading file contents.
- `src/discovery/ignore.rs` configures local ignore sources and checks enabled sources for I/O errors.
- `src/discovery/filters.rs` compiles glob and file-type filters, configures depth limits, and checks file sizes while preserving metadata errors.
- `src/error.rs` preserves the operation, path, and underlying I/O error.
- `src/input/reader.rs` opens files and yields numbered, owned lines.
- `src/search/mod.rs` exposes the matcher and result type.
- `src/input/patterns.rs` reads UTF-8 pattern files without trimming pattern text.
- `src/search/literal.rs` provides the case-sensitive in-memory literal search API.
- `src/search/pattern.rs` prepares patterns and applies case, scope, and line-selection settings.
- `src/search/regex.rs` compiles regex expressions and returns matches in original text.
- `src/search/result.rs` defines `LineMatch` and validated byte ranges in `MatchSpan`.
- `src/output/terminal.rs` writes records with optional line numbers and propagates write errors. The application flushes stdout.
- `tests/search.rs` tests the public library interface without filesystem input.
- `tests/app.rs` checks executable output, stderr, and successful exit status.
- `tests/cli.rs` checks CLI edge cases and error exit statuses.
- `tests/discovery.rs` checks recursive search, filter precedence, ignore-file failures, symlinks, and error continuation.
- `tests/fixtures/text/demo.txt` supplies a file-search fixture.

The search core performs no input or output. The caller owns the input and must keep it alive while using borrowed results.

The regex engine uses `regex` 1 to compile each pattern once and return match spans in original UTF-8 text. The prepared-pattern layer supports regex and escaped-literal modes, smart case, whole-word and whole-line scopes, multiple-pattern OR selection, inversion, and sorted, deduplicated spans with distinct overlaps preserved. The CLI uses this layer without coupling the search core to clap or filesystem input.

Pattern preparation uses `regex-syntax` 0.8 to parse and reprint expressions before adding scope boundaries. This preserves expression structure and prevents verbose-mode comments from consuming added syntax.

The pattern-file reader preserves line order, duplicates, blank patterns, spaces, and hash characters. It accepts UTF-8 input, removes LF/CRLF line endings, and fails the load on any read error.

The CLI uses clap 4 with its `derive` feature to define arguments, generate help/version output, and report syntax errors. Clap types stay outside the search core.

Discovery uses `ignore` 0.4 for traversal, local ignore matching, glob compilation, built-in file types, and symlink-loop detection. A separate metadata check enforces file-size limits on explicit roots. An additional buffered read preserves ignore-file I/O diagnostics that automatic dependency loading suppresses.

## Tests

Tests cover matching lines, absent patterns, case sensitivity, empty input, empty patterns, repeated occurrences, Unicode, and CRLF line endings.

CLI tests cover paths with spaces, usage errors, no matches, empty files and lines, missing files, directory input, invalid UTF-8, and partial output before a read error. A Windows-only test checks opening a file blocked by an exclusive handle. Unit tests cover simulated read and write failures and error sources. Unix-only tests for non-UTF-8 paths and patterns are present but have not been verified on Unix.

Parser, configuration, matcher, and executable tests cover defaults, short and long flags, combined and repeated flags, conflicting flags, Unicode case folding, smart case, word and line scopes, inversion, byte spans, zero-width matches, overlapping patterns, the `--` separator, help/version, and empty paths. Pattern-source tests cover repeated `-e`/`-f`, empty sources, CRLF, invalid UTF-8, malformed regex, argument grammar, and errors before target discovery or output.

Discovery tests cover nested and empty trees, independent line numbers, hidden entries, local ignore rules and negation, enabled-source auditing, ignore overrides, depth/size boundaries, ordered globs, file types, explicit roots, symbolic-link aliases, broken links, loops, and locked files on Windows.

Symlink tests create real links and fail if creation is unavailable. On Windows, enable Developer Mode or use an account/session with symlink-creation permission. See [Rust's Windows symlink requirements](https://doc.rust-lang.org/std/os/windows/fs/fn.symlink_file.html#limitations).

Terminal command (run from repository root)
```powershell
cargo fmt
cargo clippy --all-targets --all-features
cargo test --all-features
```

## Limitations

The in-memory literal `search` API collects results in a `Vec`. The CLI processes one line at a time; a very long line or a line with many matches can still require substantial memory. Prepared patterns compile once before target-file search. Search accepts UTF-8 content only. Multiple explicit roots, type-list output, and stdin are not implemented.

Regex matching is limited to one line at a time. Look-around and backreferences are unsupported. Pattern files are loaded into memory before search. Unicode normalization is not performed.

Hidden entries are skipped during directory search unless `--hidden` is supplied; an explicit hidden file is searched. Enabled local `.ignore`, `.gitignore`, and `.git/info/exclude` rules apply during traversal, including outside Git repositories. Explicit file roots bypass ignore exclusions but obey size limits. Parent-directory and global Git ignore rules are disabled at this stage.

Ignore syntax errors are reported. An extra buffered read detects I/O errors in enabled local ignore sources that the dependency suppresses. This check and file-size checks do not provide an atomic filesystem snapshot.

Only regular-file candidates are searched. Link following can leave the supplied directory tree and can produce duplicate underlying content through different alias paths. Broken pipes currently produce an output error and exit status `2`; graceful pipeline handling is planned.

## Planned features

- Context lines, result limits, counts, and more output modes.
- Stdin, binary detection, color, and graceful pipeline handling.
- Bounded parallel search and performance benchmarks.
- JSON Lines, statistics, configuration files, and shell completions.
- Cross-platform CI and distributable release binaries.
- Optional terminal UI and local indexing.

See `roadmap.md` for the detailed development plan.

## Development workflow

The human writes and runs implementation code, tests, configuration, and scripts. The AI teaches, reviews, supplies requested code, and maintains this README. Follow `AGENTS.md` and verify each change against its acceptance criteria.
