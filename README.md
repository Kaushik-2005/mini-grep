# mini-grep

## Project status

Active development. The CLI supports local regex and literal search in files and stdin, binary detection with a text override, path-list input, context lines, selection limits, quiet mode, file lists, counts, match-only output, filename and byte-column controls, match highlighting, headings, NUL-delimited filenames, recursive traversal, path filters, ignore controls, symbolic links, contextual error diagnostics, Ctrl+C cancellation, and graceful handling of closed stdout pipes.

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

The CLI uses regex matching by default. Use `mini-grep [OPTIONS] <PATTERN> [PATH]`, or supply patterns with `-e` and `-f` and optionally give one target path as the positional argument. Explicit filesystem targets can be files or directories. Directories are searched recursively for regular files, using buffered, line-by-line UTF-8 input. Default output is `path:text`; `-n` adds the 1-based line number as `path:line:text`. Line numbers restart for each file. Result order is unspecified. Diagnostics go to stderr. Paths remain native operating-system paths; displayed paths can replace invalid Unicode with replacement characters.

Use `mini-grep --files [OPTIONS] [PATH]` to enumerate discovered regular-file candidates without a pattern or content search. The default target is the current directory, even with redirected stdin. Target `-` is rejected; `./-` remains a filesystem path. Existing discovery filters and symlink policy apply. Candidates are not opened for content reading, so invalid UTF-8 and locked contents do not prevent enumeration when discovery succeeds.

Use `--files-from FILE` to read target paths from a line-based list. Repeat the option to combine lists, or use `--files-from -` to read paths from stdin. This works with search and `--files`; a positional target cannot be combined with path lists. Search still requires a pattern source. Stdin can supply patterns, paths, or search content, but only one of these per invocation.

Path lists accept LF and CRLF endings and an unterminated final line. Empty lines are skipped. Spaces, hash characters, duplicates, and source order are preserved. Relative paths resolve from the current working directory, not the list file's directory. Each entry can name a file or directory and uses the existing discovery filters. A listed `-` is a filesystem name. An empty list supplies no targets and does not select stdin or the current directory. Order within each discovered directory remains unspecified.

Path lists preserve native filename bytes on Unix and require UTF-8 on Windows. NUL bytes are rejected. All lists are loaded before target discovery or output, including with a zero global limit. A list-source error stops the operation without target output; target errors retain earlier results and allow later roots to continue. Selection limits and quiet stopping apply across the listed roots.

Use the explicit target `-` to search stdin. With no target, redirected stdin is selected; if stdin is a terminal, the current directory is selected instead. When `-f -` reserves stdin for patterns, an omitted search target selects the current directory instead. Empty redirected search input stays stdin and does not trigger directory search. An explicit target always wins, subject to the single-consumer rule below. Output and read diagnostics use the label `(standard input)`. The stream is processed once through the same buffered-reader search function as files. Matching, context, limits, output modes, and quiet mode retain their existing behavior. File-list modes evaluate search stdin as one input and print its label when it qualifies. Filesystem discovery filters do not apply to search stdin. An explicit filesystem path searches that path instead of piped input. Use `./-` to search a file named `-`. A pattern is still required unless `-e` or `-f` supplies patterns.

| Option | Behavior |
|---|---|
| `-i`, `--ignore-case` | Use Unicode-aware case-insensitive matching |
| `-F`, `--fixed-strings` | Select literal matching; the CLI default is regex |
| `-S`, `--smart-case` | Ignore case when no supplied pattern contains an uppercase character |
| `-w`, `--word-regexp` | Require non-word characters or line boundaries around a match |
| `-x`, `--line-regexp` | Require a match covering the entire line |
| `-e PATTERN`, `--regexp PATTERN` | Add a pattern; repeat to add more |
| `-f FILE`, `--file FILE` | Read UTF-8 patterns from a file or `-` for stdin; repeat for distinct sources |
| `--text` | Disable NUL-based binary detection for search content; UTF-8 is still required |
| `-n`, `--line-number` | Include line numbers |
| `--column` | Include one-based byte columns and imply line numbers |
| `--color auto\|always\|never` | Highlight matched text; default `auto` enables color when stdout is a terminal |
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
| `--files` | List discovered file paths without requiring patterns or reading contents |
| `--files-from FILE` | Read target paths from a file or `-` for stdin; repeat to combine lists |
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

`--color auto` highlights matches when stdout is a terminal and keeps redirected output plain. `--color always` enables highlighting even when redirected; `--color never` adds no color sequences. Matches use bold red styling where the terminal supports it. The policy uses the command-line choice and stdout terminal status; environment-variable color preferences are not consulted.

Only matched text is highlighted. Filenames, headings, numbers, context, counts, file lists, enumeration, and diagnostics retain plain formatting. Inverted selections and zero-width spans add no styling. Full-line output merges overlapping or adjacent highlighted spans without duplicating text. Match-only output retains its separate span records and original byte columns. Coloring changes neither matching nor exit status. Span validation uses checked UTF-8 byte slices; original input control characters are not escaped.

Search reads complete byte lines and checks for NUL before decoding UTF-8. By default, a NUL byte stops the affected input without a diagnostic or operational error. The affected line is not matched or printed, and later files can continue. This is a binary-content heuristic, not an encoding detector. `--text` disables the NUL check and preserves NUL bytes in valid UTF-8 content. Invalid UTF-8 on a line that reaches decoding remains a read error with status `2`.

Binary detection is streaming. Matches and context already printed remain. Earlier selections still consume selection budgets and determine exit status, even if a binary stop suppresses an unfinished count record. A binary stop does not qualify an input for `--files-without-match`. Quiet mode, file decisions, and selection limits can stop before later binary content is inspected. The policy applies equally to files and search stdin. Pattern sources and path lists retain their own validation rules. Enumeration reads no content and rejects `--text`.

`--heading` prints a file path once before its first visible line or match-only record and removes repeated filename prefixes beneath it. It retains line numbers, columns, and context markers. Visible file groups have one blank line between them, with no leading or extra trailing blank line. Context groups within a file retain `--` separators. Files without visible records produce no heading, including match-only files with only zero-width matches. Headings conflict with `-h` and are compatible with `-H`. Count and file-list modes ignore headings; quiet mode suppresses them. Read errors preserve headings and records already printed, while errors before a visible record produce no heading.

`-0` replaces the delimiter immediately after an emitted filename with one NUL byte. File-list output is `path<NUL>` with no newline. Line and match-only records retain their text newlines; numeric fields keep their usual separators. Counts become `path<NUL>count` followed by a newline. Context group breaks remain unchanged. With `-h`, records without filenames are unchanged; file-list modes still emit paths. NUL output conflicts with headings. Quiet mode suppresses records, and diagnostics remain readable. The flag delimits filenames only; it does not escape searched text or replace its line endings.

NUL filename output preserves native bytes on Unix. On Windows it uses UTF-8 and reports an output error for a filename that cannot be represented losslessly as Unicode. Ordinary displayed paths retain their existing replacement-character policy.

Patterns from all sources are combined with OR. A selected line is printed once. Inversion applies after combining patterns. Pattern files preserve spaces, hash characters, duplicates, and blank lines; LF and CRLF endings are removed. An empty file supplies no patterns and selects no lines. A blank line supplies an empty pattern and matches every line in the default scope. `-v` reverses these selections. To read a pattern file named `-`, use a path such as `./-`.

`-f -` reads UTF-8 patterns from stdin through EOF, using the same rules as pattern files. It can combine with explicit patterns and ordinary pattern files. Repeated `-f -` and simultaneous search target `-` are usage errors: stdin has one consumer. An omitted target with stdin patterns selects the current directory, regardless of terminal status. Pattern-read errors identify `(standard input)`. All patterns are loaded and compiled before discovery or output, even with `--max-results 0`. Empty pattern stdin supplies zero patterns; a blank line supplies one empty pattern.

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

Enumeration prints one path per line, or NUL-terminated paths with `-0`. Line numbers, columns, headings, and filename-visibility flags do not alter enumeration records. Quiet mode suppresses paths and stops at the first candidate. Discovery errors are reported while enumeration continues, and errors already encountered retain status `2` after quiet stopping. Otherwise status is `0` if any candidate exists and `1` if none exists. Enumeration rejects pattern sources, matching flags, context options, search limits, and competing output modes. Existing formatting conflicts, including headings with NUL output, remain enforced.

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
| `2` | Usage, pattern, discovery, open, read, output, signal-setup error, or cancellation |

For search, all pattern files are loaded and all patterns are compiled before target discovery. A pattern-file or compilation failure stops execution before search output. Discovery, target-file open, and target-file read errors are reported while other available candidates continue. A read error stops the affected file. Previously printed matches remain on stdout. Any operational error makes the final status `2`, even if selected lines were printed. Output errors stop execution. Invalid UTF-8 that reaches text decoding is a read error; NUL detection can stop a line before decoding. A non-UTF-8 pattern is a usage error. Enumeration bypasses pattern loading and content reading.

When a stdout write or flush reports a broken pipe, search or enumeration stops immediately without a broken-pipe diagnostic. Pending diagnostics are flushed. The exit status is `0` unless an earlier operational error requires `2`. This graceful completion also applies to zero-count output; it does not prove that a match was selected. Other stdout errors and stderr failures return `2`. Help and version output also complete gracefully when stdout reports a broken pipe. Input and targets beyond the stopping point are not checked.

The first Ctrl+C requests cooperative cancellation. Search, enumeration, and source loading check the shared request at work and buffered-read boundaries. Observed cancellation reports `mini-grep: operation cancelled` and returns `2`. Records already written remain; an interrupted file does not produce a completed count or qualify for `--files-without-match`. Cancellation does not interrupt an active blocking I/O call or a running regex operation. If the process remains blocked, a second Ctrl+C requests forced termination with status `2`. Forced termination skips Rust destructors and does not guarantee buffered output or a diagnostic.

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

Search piped input with an explicit stdin target.

Terminal command (run from repository root)
```powershell
"Rust", "Go" | cargo run -- -n Rust -
```

Select redirected stdin automatically by omitting the target.

Terminal command (run from repository root)
```powershell
"Rust", "Go" | cargo run -- -n Rust
```

Read patterns from stdin and search an explicit file.

Terminal command (run from repository root)
```powershell
"Rust", "design" | cargo run -- -n -f - tests/fixtures/text/demo.txt
```

Enumerate discovered Rust source files without searching their contents.

Terminal command (run from repository root)
```powershell
cargo run -- --files --type rust .
```

Read a target path from stdin and search that file.

Terminal command (run from repository root)
```powershell
"tests/fixtures/text/demo.txt" | cargo run -- -n Rust --files-from -
```

Force match highlighting, including when stdout is redirected.

Terminal command (run from repository root)
```powershell
cargo run -- --color=always -n Rust tests/fixtures/text/demo.txt
```

Search valid UTF-8 content even when it contains NUL bytes.

Terminal command (run from repository root)
```powershell
cargo run -- --text -n Rust tests/fixtures/text/demo.txt
```

## Design summary

- `src/lib.rs` exposes the search module.
- `src/main.rs` calls `run`, reports errors, and selects the exit status.
- `src/cli/args.rs` defines clap syntax and help text.
- `src/cli/config.rs` converts parsed arguments into validated settings.
- `src/app.rs` connects configuration, input, matching, and output.
- Enumeration dispatch occurs before pattern loading and stdin detection. Its application helper connects the existing walker directly to path rendering.
- Per-file settings and the shared global selection budget stay in `src/app.rs`; matchers and context selection do not own search limits.
- `src/discovery/walker.rs` yields regular-file candidates and traversal errors without reading file contents.
- `src/discovery/ignore.rs` configures local ignore sources and checks enabled sources for I/O errors.
- `src/discovery/filters.rs` compiles glob and file-type filters, configures depth limits, and checks file sizes while preserving metadata errors.
- `src/error.rs` preserves the operation, path, and underlying I/O error, and distinguishes result-output failures from diagnostic failures.
- `src/input/reader.rs` opens files, reads byte lines, and yields numbered UTF-8 text or a terminal binary event. LF/CRLF endings are removed; read and decoding failures stop the iterator.
- `src/input/binary.rs` owns the NUL-based skip/text policy. Detection precedes decoding; the application handles binary stops without coupling pure matching to input classification. No new dependency is required.
- Its cancellation-aware reader checks the shared token before reads for search content, pattern sources, and path lists. Application code translates cancellation into an application error.
- `src/execution/cancel.rs` owns an `Arc<AtomicBool>` cancellation token and Ctrl+C handler registration. Search settings borrow the token; pure matching remains independent of cancellation. The `ctrlc` 3 dependency supplies cross-platform signal handling without application-defined unsafe code.
- `src/input/stdin.rs` locks stdin, defines its display label, and resolves optional targets from terminal status. The application performs terminal detection. File opening and shared buffered-reader processing are separate functions in `src/app.rs`.
- `src/search/mod.rs` exposes the matcher and result type.
- `src/input/patterns.rs` reads UTF-8 pattern files or locked stdin without trimming pattern text. Validated pattern-source variants and application target selection prevent consuming stdin twice.
- `src/input/paths.rs` reads line-based path lists and preserves platform-specific filename rules. The application loads lists before discovery and shares selection budgets across their roots. This requires no new dependency.
- `src/search/literal.rs` provides the case-sensitive in-memory literal search API.
- `src/search/pattern.rs` prepares patterns and applies case, scope, and line-selection settings.
- `src/search/regex.rs` compiles regex expressions and returns matches in original text.
- `src/search/context.rs` retains preceding lines and emits selected lines, context lines, and group breaks without I/O.
- `src/search/result.rs` defines borrowed `LineMatch` results, validated byte ranges in `MatchSpan`, and internal owned line events.
- `src/output/terminal.rs` renders selected lines, context lines, and group breaks, and propagates write errors. The application flushes stdout.
- `src/output/mod.rs` defines the internal output mode. File qualification stays in the application; the terminal module renders qualifying paths.
- `TerminalOptions` groups filename, line-number, column, NUL, and color formatting. Line, context, and match-only rendering share prefix formatting; diagnostics stay separate.
- `src/output/color.rs` resolves the color policy, selects a plain or styled stdout writer, validates spans, and merges highlighted ranges. The `anstream` 1 dependency adapts enabled styling for terminal capabilities, including Windows console fallback. Disabled styling retains the raw writer. Search logic remains independent of color and clap.
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

Color tests cover parser choices, configuration conversion, terminal-policy combinations, Unicode byte ranges, invalid and unsorted spans, overlaps, adjacent ranges, zero-width matches, and write failures. Executable tests cover redirected default/auto/never output, forced ANSI output, plain prefixes and context, match-only byte columns, overlap handling, and modes that remain plain. A Windows terminal screenshot confirmed highlighted matches for auto/always and plain output for never. Linux, macOS, and legacy Windows console fallback remain unverified.

Binary tests cover NUL at different positions, detection before UTF-8 decoding, line numbering and endings, text override behavior, invalid UTF-8 under both policies, late binary content preserving printed context, unfinished counts, file qualification, stdin, and early exits that leave later bytes unchecked. A cross-root test verifies that binary input does not block later targets and remains eligible for enumeration. Configuration tests reject `--text` with enumeration.

Heading tests cover lazy emission, repeated records, blank-line separation, files without visible records, context groups, line numbers and columns, match-only spans, zero-width selections, quiet and ignored modes, limits, partial output before read failures, and write failures in initial and later groups.

NUL-output tests assert exact bytes across output modes, context separators, filename suppression, quiet mode, partial results, readable diagnostics, multiple file-list records, and write failures. A Windows unit test rejects unpaired UTF-16 surrogates without partial filename output. Unix-only unit and executable tests preserve non-UTF-8 filename bytes and embedded newlines; these still require a Unix verification run.

Stdin executable tests supply bytes through a child-process pipe and close it to signal EOF. They cover explicit `-`, Unicode and CRLF, empty input, no selections, repeated patterns, output modes, context, columns, headings, NUL output, quiet mode, limits, and invalid UTF-8 with source diagnostics and partial-output rules.

Automatic-target tests cover omitted targets with positional or explicit patterns, empty redirected input, explicit-file precedence, and literal dash filenames. Unit tests cover terminal-to-current-directory fallback and preservation of explicit target spelling. An interactive terminal fallback has not been recorded as manually verified.

Stdin-pattern tests cover explicit file targets, the current-directory default, mixed argument/file/stdin sources, empty and blank sources with inversion, invalid UTF-8, regex errors before output and zero limits, single-consumer usage errors, and a relative pattern file named `-`.

Enumeration tests cover pattern-free grammar, invalid targets, incompatible search options, empty trees, unread invalid UTF-8, existing discovery filters, NUL records, quiet stopping, earlier ignore-file errors, missing targets, current-directory selection despite redirected stdin, and exclusively locked Windows files. Formatting flags leave enumeration path records unchanged.

Path-list tests cover file and stdin sources, repeated lists, source order and duplicates, LF/CRLF, empty lists, whitespace, NUL rejection, current-directory resolution, literal dash files and directories, discovery filters, shared limits, quiet stopping, source failures before output, and recovery across target errors. Unix-only native-byte tests still require Unix verification.

Closed-pipe executable tests close stdout before supplying target paths. They cover search, zero-count output, enumeration, NUL records, earlier target errors, and stopping before later targets during larger output. Unit tests cover ordinary stdout failures, diagnostic flush failures, and stream-specific classification of help/version printing failures. Cross-platform pipe behavior still requires Linux and macOS verification.

Cancellation unit tests cover initial and repeated requests, shared state across threads, independent tokens, cancelled source loading before file opening, reader checks without consuming input, and cancellation at EOF. EOF tests preserve printed lines while rejecting unfinished counts and file qualification. A reported Windows manual stdin run confirmed cooperative cancellation: it printed `mini-grep: operation cancelled` and returned status `2` after one Ctrl+C. Forced termination and real-signal behavior on Linux and macOS remain unverified.

Symlink tests create real links and fail if creation is unavailable. On Windows, enable Developer Mode or use an account/session with symlink-creation permission. See [Rust's Windows symlink requirements](https://doc.rust-lang.org/std/os/windows/fs/fn.symlink_file.html#limitations).

Terminal command (run from repository root)
```powershell
cargo fmt
cargo clippy --all-targets --all-features
cargo test --all-features
```

## Limitations

The in-memory literal `search` API collects results in a `Vec`. The CLI processes one line at a time; a very long line or a line with many matches can still require substantial memory. Prepared patterns compile once before target search. Search accepts UTF-8 content only. Multiple positional targets and type-list output are not implemented; use path lists for multiple roots. Stdin pattern sources and path lists are loaded fully into memory before target search.

There is no encoding selector or transcoding. `--text` does not enable UTF-16, legacy encodings, or arbitrary-byte matching. Binary detection checks complete lines, so even a NUL-containing line can require a large allocation before it is classified. Invalid bytes without a NUL marker are read errors, not automatic binary skips.

Path-list input is line-based and cannot represent filenames containing newlines. `-0` changes output only; NUL-delimited output cannot be used directly as path-list input.

Regex matching is limited to one line at a time. Look-around and backreferences are unsupported. Pattern files are loaded into memory before search. Unicode normalization is not performed.

Before-context buffers owned lines; memory use depends on the requested count and line lengths. A large context count can retain much of a file.

NUL-delimited filenames are not a complete structured output format. With `--text`, searched text can contain NUL bytes; use file-list modes when consuming a stream of NUL-terminated paths. On Windows, NUL filename output rejects paths containing unpaired UTF-16 surrogates.

Hidden entries are skipped during directory search unless `--hidden` is supplied; an explicit hidden file is searched. Enabled local `.ignore`, `.gitignore`, and `.git/info/exclude` rules apply during traversal, including outside Git repositories. Explicit file roots bypass ignore exclusions but obey size limits. Parent-directory and global Git ignore rules are disabled at this stage.

Ignore syntax errors are reported. An extra buffered read detects I/O errors in enabled local ignore sources that the dependency suppresses. This check and file-size checks do not provide an atomic filesystem snapshot.

Only regular-file candidates are searched. Link following can leave the supplied directory tree and can produce duplicate underlying content through different alias paths. Graceful pipe closure applies specifically to errors classified as `BrokenPipe`; other I/O error kinds remain operational errors.

## Planned features

- Bounded parallel search and performance benchmarks.
- JSON Lines, statistics, configuration files, and shell completions.
- Cross-platform CI and distributable release binaries.
- Optional terminal UI and local indexing.

See `roadmap.md` for the detailed development plan.

## Development workflow

The human writes and runs implementation code, tests, configuration, and scripts. The AI teaches, reviews, supplies requested code, and maintains this README. Follow `AGENTS.md` and verify each change against its acceptance criteria.
