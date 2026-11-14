# mini-grep

## Project status

Active development. The CLI supports local regex and literal search, context lines, selection limits, quiet mode, file lists, counts, match-only output, filename and byte-column controls, headings, NUL-delimited filenames, recursive traversal, path filters, ignore controls, symbolic links, and contextual error diagnostics.

The developer reports passing Windows formatting, Clippy, and test checks, including matching, discovery, context, limits, and all implemented output modes. Unix-only tests remain unverified on Unix.

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
| `--column` | Include one-based byte columns and imply line numbers |
| `-H`, `--with-filename` | Include filename prefixes; this is the default |
| `-h`, `--no-filename` | Omit filename prefixes from line, match-only, and count output |
| `--heading` | Group visible line and match-only records beneath each file path |
| `-0`, `--null` | Write a NUL byte after each emitted filename |
| `-v`, `--invert-match` | Select lines that do not match |
| `-A NUM`, `--after-context NUM` | Include up to this many lines after each selected line |
| `-B NUM`, `--before-context NUM` | Include up to this many lines before each selected line |
| `-C NUM`, `--context NUM` | Set both context counts |
| `-m NUM`, `--max-count NUM` | Limit selected lines per file |
| `--max-results NUM` | Limit selected lines across the search |
| `-q`, `--quiet` | Print no search results and stop at the first qualifying result |
| `-l`, `--files-with-matches` | Print each file path once if it contains a selected line |
| `--files-without-match` | Print each file path once if it contains no selected lines |
| `-c`, `--count` | Print the selected-line count for each evaluated file |
| `--count-matches` | Print the match-span count for each evaluated file |
| `-o`, `--only-matching` | Print each nonempty match span as a separate record |
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

Compatible search flags can be combined, such as `-in`. Repeated boolean flags such as `-ii` are rejected. `-i` conflicts with `-S`; `-w` conflicts with `-x`; `-H` conflicts with `-h`. Help is available through `--help`; `-h` controls filenames. Use `--` before a positional pattern that begins with a hyphen, or use `--regexp=-needle`.

Filenames are included by default, even for one explicit file. `-h` removes filename prefixes without changing matching, limits, or exit status. Selected lines and matched spans become `text`, or `line:text` with `-n`. Context lines become `text`, or `line-text` with `-n`. Counts become plain numbers and still ignore line-number options. Context group separators remain `--`. File-list modes always print paths, and diagnostics retain path context. Quiet mode still suppresses result output.

`--column` adds the first match's one-based byte column to a selected-line record: `path:line:column:text`. Match-only records use each span's own start column. Inverted selections without spans use column `1`; context lines use column `1` with `-` separators. `-h` removes only the filename field. For `é Rust`, `Rust` begins at byte column `4`; columns do not measure characters or terminal display cells. A zero-width match at the end of a line has column equal to its byte length plus one. Count and file-list modes ignore column formatting, and quiet mode suppresses output as usual.

`--heading` prints a file path once before its first visible line or match-only record and removes repeated filename prefixes beneath it. It retains line numbers, columns, and context markers. Visible file groups have one blank line between them, with no leading or extra trailing blank line. Context groups within a file retain `--` separators. Files without visible records produce no heading, including match-only files with only zero-width matches. Headings conflict with `-h` and are compatible with `-H`. Count and file-list modes ignore headings; quiet mode suppresses them. Read errors preserve headings and records already printed, while errors before a visible record produce no heading.

`-0` replaces the delimiter immediately after an emitted filename with one NUL byte. File-list output is `path<NUL>` with no newline. Line and match-only records retain their text newlines; numeric fields keep their usual separators. Counts become `path<NUL>count` followed by a newline. Context group breaks remain unchanged. With `-h`, records without filenames are unchanged; file-list modes still emit paths. NUL output conflicts with headings. Quiet mode suppresses records, and diagnostics remain readable. The flag delimits filenames only; it does not escape searched text or replace its line endings.

NUL filename output preserves native bytes on Unix. On Windows it uses UTF-8 and reports an output error for a filename that cannot be represented losslessly as Unicode. Ordinary displayed paths retain their existing replacement-character policy.

Patterns from all sources are combined with OR. A selected line is printed once. Inversion applies after combining patterns. Pattern files preserve spaces, hash characters, duplicates, and blank lines; LF and CRLF endings are removed. An empty file supplies no patterns and selects no lines. A blank line supplies an empty pattern and matches every line in the default scope. `-v` reverses these selections. `-f -` is rejected because stdin is not supported. To read a file named `-`, use a path such as `./-`.

Smart case checks the raw text of every supplied pattern, including regex syntax. Any uppercase character makes the default case setting sensitive for all patterns. Regex inline flags can override that setting within an expression. Whole-word boundaries use Unicode word characters, including letters, digits, combining marks, and underscores. Whole-line matching excludes the line ending. An empty pattern with `-x` matches only empty lines.

Context counts accept nonnegative integers and default to zero. Explicit `-A` and `-B` values override their side of `-C`, regardless of argument order; explicit zero disables that side. Overlapping or adjacent context windows form one group, and each line appears once. Separate groups within a file have a `--` separator with no leading or trailing separator. Selected lines use `path:line:text`; context lines use `path-line-text`. Without `-n`, the line-number field is omitted. Context state resets for each file, with no separator added between files. With `-v`, context surrounds inverted selections. Context lines do not change the exit status. No selections produce no context output. Read errors preserve context already printed.

Selection limits accept nonnegative integers. Omitted limits are unlimited. Each selected line counts once, regardless of its match count; with `-v`, inverted selections count. Context lines do not count. `-m` resets for each file, while `--max-results` is shared across files. When either budget is exhausted, the current file emits only the requested after-context and then stops. Matching lines in that trailing context remain context and do not extend the window. An exhausted global budget also stops traversal before the next candidate is requested. Because traversal order is unspecified, the files included under a global limit can vary.

`-m 0` skips opening candidate files. `--max-results 0` returns status `1` before target discovery, after loading and compiling patterns. Errors encountered before stopping still produce status `2`, including errors while reading required trailing context. Input beyond the stopping point is not checked.

Quiet mode writes no search results and does not flush stdout. In line-output mode, it stops at the first permitted selected line, without reading trailing context or requesting another candidate. In file-list modes, it stops at the first qualifying file. Line numbers and context options are accepted but produce no output. Inversion and selection limits still apply. Diagnostics remain on stderr, and pattern validation still occurs before search. An error already encountered keeps final status `2`, even if a later file qualifies. Otherwise, status is `0` for a result or `1` for none. Unread input is not checked. Explicit help and version requests retain their normal output.

File-list modes print paths only, one per line, and ignore line numbers and context options. `-l` conflicts with `--files-without-match`. A selected line settles either file-list decision immediately: `-l` prints the path, while `--files-without-match` excludes it. Empty files qualify for `--files-without-match`. Qualifying a file without selections requires successful EOF; an open or read failure does not qualify it. With `-v`, both modes use inverted line selection. Quiet mode suppresses paths; with `--files-without-match`, it must reach EOF in a qualifying file before stopping.

Zero selection limits produce no file-list output. `--max-results` still counts selected lines, not printed paths. A file-list scan stops at its first selection, so it consumes at most one selection from that budget per file. The global limit can stop discovery before later files without selections are evaluated. Without an error, file-list status is `0` when at least one file qualifies and `1` otherwise.

Count modes print `path:count` once per successfully evaluated file, including zero. `-c` counts selected lines; `--count-matches` counts their match spans. One line containing `Rust Rust` contributes one selected line and two match spans. Both count modes conflict with each other and with file-list modes. `-c` supports inversion; `--count-matches` conflicts with `-v`.

Occurrence counts use the existing span rules: identical ranges count once, distinct overlaps count separately, and zero-width matches count. Context and line-number options do not affect count output. Limits still count selected lines, and each permitted line contributes all its spans. Count modes stop immediately at an exhausted limit without reading trailing context. Zero limits skip files and produce no count records for them.

A file read error produces a diagnostic and no partial count for that file. Completed records from other files remain. Quiet mode suppresses count records and stops at the first permitted selected line. Without errors, count-mode status is `0` if any line was selected and `1` otherwise; printing zero counts does not imply a successful selection.

Match-only mode prints `path:matched-text`, or `path:line:matched-text` with `-n`. Each nonempty span produces one record using the original text and case. Spans are sorted by start and end; identical ranges are removed and distinct overlaps remain. Zero-width spans produce no records, but their selected lines still affect exit status and selection limits. A search can therefore return `0` without printing records.

Match-only mode conflicts with inversion, count modes, and file-list modes. Context options are accepted but ignored. Limits count selected lines, and every nonempty span on a permitted line is printed. An exhausted limit stops without reading trailing context. Quiet mode suppresses records and retains immediate stopping. Read errors preserve already printed spans and return status `2`.

`--no-ignore` takes precedence over `--no-ignore-vcs` in either argument order. These switches do not enable hidden entries; add `--hidden` for that. Depth and size limits accept nonnegative integers only. Size suffixes such as `1KB` are not supported. A directory root with maximum depth `0` yields no candidate files. An explicit file root remains at depth `0` and still obeys the size limit.

Globs are case-sensitive, independently of `-i`. The last matching glob wins. With positive globs, files must match an inclusion; with only exclusions, other files remain eligible. Anchored globs are relative to the directory root, or the parent directory of an explicit file root. Explicit file roots obey globs. Positive globs can override ignore and hidden-file exclusions, but cannot restore traversal through an excluded directory. Depth and size limits still apply. Invalid globs fail before search output.

File types use the definitions bundled with `ignore`, such as `rust` and `toml`. Multiple inclusions form a union. Type exclusions win in either argument order. With no type inclusions, other types remain eligible. Type restrictions apply to explicit roots and cannot be bypassed by glob inclusions. Selecting a type does not itself include hidden or ignored files. Unknown type names fail before search output.

Directory searches skip symbolic links by default. An explicit symlink root requires `--follow`. When following links, output and path filters use the alias path, and size limits use the target file's metadata. Different aliases can search the same underlying file more than once. Broken links and directory loops produce diagnostics and final status `2`; other files continue.

CLI case-insensitive matching uses Unicode simple case folding without normalization. For example, `CAFÉ` matches `café`, but `STRASSE` does not match `Straße`. Output preserves original line text. In the default scope, an inverted empty pattern selects no lines.

Each pattern yields nonoverlapping matches. Spans from different patterns are sorted and identical ranges are removed; distinct overlaps remain. Offsets refer to bytes in the original line, and zero-width matches are retained internally. Highlighting is not implemented yet.

| Exit status | Meaning |
|---|---|
| `0` | At least one selected line or qualifying file, or successful help/version output |
| `1` | No selected lines or qualifying files for the requested mode |
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

Show one line before and after each selected line.

Terminal command (run from repository root)
```powershell
cargo run -- -n -C 1 Rust tests/fixtures/text/demo.txt
```

Select at most one line per file and three lines across the search.

Terminal command (run from repository root)
```powershell
cargo run -- -n -m 1 --max-results 3 Rust tests/fixtures/text
```

Check for a selected line without printing search results. Inspect the process exit status.

Terminal command (run from repository root)
```powershell
cargo run -- --quiet Rust tests/fixtures/text/demo.txt
```

List files that contain a selected line.

Terminal command (run from repository root)
```powershell
cargo run -- -l Rust tests/fixtures/text
```

List files that contain no selected lines.

Terminal command (run from repository root)
```powershell
cargo run -- --files-without-match Rust tests/fixtures/text
```

Count match occurrences in each file.

Terminal command (run from repository root)
```powershell
cargo run -- --count-matches Rust tests/fixtures/text
```

Print individual matched spans with their original line numbers.

Terminal command (run from repository root)
```powershell
cargo run -- -no Rust tests/fixtures/text/demo.txt
```

Print matched spans without filename prefixes.

Terminal command (run from repository root)
```powershell
cargo run -- -ho Rust tests/fixtures/text/demo.txt
```

Print matched spans with line numbers and byte columns.

Terminal command (run from repository root)
```powershell
cargo run -- --column -o Rust tests/fixtures/text/demo.txt
```

Group recursive search results beneath file headings.

Terminal command (run from repository root)
```powershell
cargo run -- --heading -n Rust tests/fixtures/text
```

Emit selected file paths terminated by NUL bytes for a byte-aware consumer.

Terminal command (run from repository root)
```powershell
cargo run -- -l0 Rust tests/fixtures/text
```

## Design summary

- `src/lib.rs` exposes the search module.
- `src/main.rs` calls `run`, reports errors, and selects the exit status.
- `src/cli/args.rs` defines clap syntax and help text.
- `src/cli/config.rs` converts parsed arguments into validated settings.
- `src/app.rs` connects configuration, input, matching, and output.
- Per-file settings and the shared global selection budget stay in `src/app.rs`; matchers and context selection do not own search limits.
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
- `src/search/context.rs` retains preceding lines and emits selected lines, context lines, and group breaks without I/O.
- `src/search/result.rs` defines borrowed `LineMatch` results, validated byte ranges in `MatchSpan`, and internal owned line events.
- `src/output/terminal.rs` renders selected lines, context lines, and group breaks, and propagates write errors. The application flushes stdout.
- `src/output/mod.rs` defines the internal output mode. File qualification stays in the application; the terminal module renders qualifying paths.
- `TerminalOptions` groups filename, line-number, column, and NUL formatting. Line, context, and match-only rendering share prefix formatting; diagnostics stay separate.
- A shared filename writer handles displayed delimiters and platform-aware NUL filename bytes. File-list, count, line, and match-only modes use it without new dependencies.
- `HeadingState` tracks visible file groups in the terminal module. The application marks file boundaries and requests a heading only before visible records.
- Count accumulation stays in the application and uses `u128`; the terminal module renders count records and preserves write errors.
- Match-only rendering borrows original line text and uses checked byte slices for each span, without cloning substrings or repeating matching.
- `tests/search.rs` tests the public library interface without filesystem input.
- `tests/app.rs` checks executable output, stderr, and successful exit status.
- `tests/cli.rs` checks CLI edge cases and error exit statuses.
- `tests/discovery.rs` checks recursive search, filter precedence, ignore-file failures, symlinks, and error continuation.
- `tests/fixtures/text/demo.txt` supplies a file-search fixture.

The search core performs no input or output. The caller owns the input and must keep it alive while using borrowed results.

The streaming context selector owns buffered line text. It retains at most the requested number of preceding lines between calls. It emits after-context as input arrives and merges overlapping or adjacent groups. The application supplies selection decisions and paths; output handles formatting. No new dependency is required for context support.

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

Context tests cover zero counts, before-only and after-only selection, file boundaries, overlapping and adjacent windows, separate groups, Unicode text and byte spans, rendering failures, option precedence, inversion, per-file reset, no selections, and partial context output before read errors.

Limit tests cover unlimited defaults, zero, selected-line counts, per-file resets, shared global budgets, inversion, trailing context, pattern validation before a zero global limit, and read errors before versus beyond the stopping point.

Quiet-mode tests cover short, long, and combined flags, selection and no selection, inversion, zero limits, skipped trailing context, unread trailing errors, pattern errors, and an earlier discovery error followed by a selection.

File-list tests cover conflicting flags, matching and nonmatching files, empty files, path-only rendering, write failures, inversion, quiet qualification, read errors, early decisions, directory traversal, and zero and global selection limits.

Count tests cover selected lines versus occurrences, zero counts, empty files, duplicate patterns, distinct overlapping spans, zero-width matches, inversion, quiet mode, mode conflicts, write failures, limits without trailing context, read failures without partial counts, completed records from other files, and shared global budgets.

Match-only tests cover short and long flags, original Unicode text and byte ranges, repeated matches, line numbers, duplicate and overlapping spans, skipped zero-width records, selected zero-width lines consuming limits, quiet mode, no selections, write failures, incompatible span/text pairs, partial output before read errors, and shared limits across files.

Filename-control tests cover defaults, short and long forms, conflicting flags, `--help`, selected and context lines, count and match-only output, explicit inclusion, file-list paths, and diagnostic paths with and without quiet mode.

Column tests cover implied line numbers, multibyte prefixes, distinct span columns, filename suppression, context columns, inverted selections without spans, zero-width starts and ends, and ignored formatting in count, file-list, and quiet modes.

Heading tests cover lazy emission, repeated records, blank-line separation, files without visible records, context groups, line numbers and columns, match-only spans, zero-width selections, quiet and ignored modes, limits, partial output before read failures, and write failures in initial and later groups.

NUL-output tests assert exact bytes across output modes, context separators, filename suppression, quiet mode, partial results, readable diagnostics, multiple file-list records, and write failures. A Windows unit test rejects unpaired UTF-16 surrogates without partial filename output. Unix-only unit and executable tests preserve non-UTF-8 filename bytes and embedded newlines; these still require a Unix verification run.

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

Before-context buffers owned lines; memory use depends on the requested count and line lengths. A large context count can retain much of a file.

NUL-delimited filenames are not a complete structured output format. Searched text can contain NUL bytes; use file-list modes when consuming a stream of NUL-terminated paths. On Windows, NUL filename output rejects paths containing unpaired UTF-16 surrogates.

Hidden entries are skipped during directory search unless `--hidden` is supplied; an explicit hidden file is searched. Enabled local `.ignore`, `.gitignore`, and `.git/info/exclude` rules apply during traversal, including outside Git repositories. Explicit file roots bypass ignore exclusions but obey size limits. Parent-directory and global Git ignore rules are disabled at this stage.

Ignore syntax errors are reported. An extra buffered read detects I/O errors in enabled local ignore sources that the dependency suppresses. This check and file-size checks do not provide an atomic filesystem snapshot.

Only regular-file candidates are searched. Link following can leave the supplied directory tree and can produce duplicate underlying content through different alias paths. Broken pipes currently produce an output error and exit status `2`; graceful pipeline handling is planned.

## Planned features

- Stdin, binary detection, color, and graceful pipeline handling.
- Bounded parallel search and performance benchmarks.
- JSON Lines, statistics, configuration files, and shell completions.
- Cross-platform CI and distributable release binaries.
- Optional terminal UI and local indexing.

See `roadmap.md` for the detailed development plan.

## Development workflow

The human writes and runs implementation code, tests, configuration, and scripts. The AI teaches, reviews, supplies requested code, and maintains this README. Follow `AGENTS.md` and verify each change against its acceptance criteria.
