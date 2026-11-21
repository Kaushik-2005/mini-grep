# mini-grep

## Overview

mini-grep is a standalone Rust command-line tool for local text search. It searches UTF-8 files, directory trees, and standard input. It supports regular expressions (regex), literal patterns, context lines, and several output modes. The project develops Rust skills through a modular search engine and measured performance work.

## Project status

Core development is largely complete. The project has passed formatting, linting, the full test suite, and all 65 benchmark cases on Windows.

Current work focuses on identifying performance bottlenecks and exploring further optimizations to improve search speed, memory efficiency, and resource utilization.

Remaining development includes CLI utilities, cross-platform CI for Windows, Linux, and macOS, release preparation, versioning, and an optional terminal user interface (TUI).

I will make proper documentation in a separate `\docs\` folder

## Features

- Regex and literal search with Unicode-aware case options.
- Multiple patterns from arguments, files, or standard input.
- Recursive discovery with ignore rules, globs, file types, and size limits.
- Context lines, per-file limits, global limits, and quiet searches.
- Line, match-only, count, file-list, and enumeration output.
- Filename controls, byte columns, highlighting, headings, and NUL-delimited filenames (zero-byte delimiters).
- Standard input and line-based target-path lists.
- Bounded parallel file search, optional path sorting, and Ctrl+C cancellation.
- Recoverable file diagnostics and graceful handling of broken stdout pipes.

## Prerequisites

Install stable Rust with Cargo. The package uses Rust edition 2024. A minimum supported Rust version is not declared.

Examples use PowerShell and run from the repository root. Windows memory scripts require PowerShell 7 and a policy that permits local scripts. Benchmark comparisons also require ripgrep. The three-tool source comparison requires GNU grep.

## Installation

Build the release executable.

Terminal command (run from repository root)

```powershell
cargo build --release
```

On Windows, the executable is `target\release\mini-grep.exe`. On Unix-like systems, the expected path is `target/release/mini-grep`.

To install the executable into Cargo's binary directory, run:

Terminal command (run from repository root)

```powershell
cargo install --path .
```

## Quick start

Search the included fixture and print line numbers.

Terminal command (run from repository root)

```powershell
cargo run -- -n Rust tests/fixtures/text/demo.txt
```

Expected output (illustrative)

```text
tests/fixtures/text/demo.txt:2:Rust makes ownership explicit.
tests/fixtures/text/demo.txt:4:Rust rewards careful design.
```

Use `cargo run --` during development. After installation, use `mini-grep` directly.

## CLI reference

Supported forms:

- `mini-grep [OPTIONS] <PATTERN> [PATH]`
- `mini-grep [OPTIONS] (-e <PATTERN> | -f <FILE>)... [PATH]`
- `mini-grep --files [OPTIONS] [PATH]`

Only one positional target is accepted. Use `--files-from` for multiple roots. When `-e` or `-f` supplies patterns, the optional positional argument is the target.

Show the complete generated help.

Terminal command (run from repository root)

```powershell
cargo run -- --help
```

### Matching and pattern sources

| Option | Behavior |
|---|---|
| `-F, --fixed-strings` | Match literal text. The default is regex. |
| `-i, --ignore-case` | Use Unicode-aware case-insensitive matching. |
| `-S, --smart-case` | Ignore case when no supplied pattern contains an uppercase character. |
| `-w, --word-regexp` | Require word-boundary conditions around a match. |
| `-x, --line-regexp` | Require a match covering the entire line. |
| `-v, --invert-match` | Select nonmatching lines. |
| `-e, --regexp PATTERN` | Add a pattern. Repeat to add more. |
| `-f, --file FILE` | Read UTF-8 patterns from a file. Use `-` for stdin. |
| `--text` | Disable NUL-based binary detection. UTF-8 is still required. |

Patterns are combined with OR selection. Pattern files preserve spaces, hash characters, duplicates, and blank patterns. An empty file contributes no patterns; a blank line contributes an empty pattern.

All patterns are loaded and compiled before target discovery. A source or syntax error stops the search before result output.

Case-insensitive matching uses Unicode simple case folding without normalization. Matches preserve the original text. Regex matching operates on individual lines; look-around and backreferences are unsupported.

Each pattern yields nonoverlapping spans. Combined spans are sorted and deduplicated. Distinct overlaps between patterns remain. Zero-width spans are retained internally.

`-i` conflicts with `-S`. `-w` conflicts with `-x`. Use `--` before a positional pattern beginning with a hyphen, or use `--regexp=-needle`.

### Discovery and targets

| Option | Behavior |
|---|---|
| `--files` | Enumerate candidate paths without a pattern or content search. |
| `--files-from FILE` | Read target paths from a file. Repeat to combine lists. Use `-` for stdin. |
| `--hidden` | Include hidden files and directories. |
| `--no-ignore` | Disable supported ignore-file rules and their read checks. |
| `--no-ignore-vcs` | Disable Git ignore rules while retaining `.ignore` rules. |
| `-g, --glob GLOB` | Include matching paths. Prefix with `!` to exclude. Later matches win. |
| `--type TYPE` | Include a built-in file type. Repeat to form a union. |
| `--type-not TYPE` | Exclude a built-in file type. Exclusions take precedence. |
| `--max-depth DEPTH` | Limit traversal depth. The root is depth zero. |
| `--max-filesize BYTES` | Set an inclusive byte-size limit, including explicit file roots. |
| `-L, --follow` | Follow symbolic links. Report broken links and directory loops. |

Directory search visits regular-file candidates recursively. Local `.ignore`, `.gitignore`, and `.git/info/exclude` rules apply, including outside Git repositories. Parent-directory and global Git ignore rules are disabled.

Explicit file roots bypass ignore exclusions. They still obey glob, type, and size filters. Positive globs can override hidden and ignore exclusions. They cannot restore traversal through an excluded directory. Type, depth, and size restrictions still apply.

`--no-ignore` takes precedence over `--no-ignore-vcs`. Neither option enables hidden entries by itself. Size limits accept byte counts without suffixes.

Discovered symlinks are skipped by default. An explicit symlink root requires `--follow`. Aliases can search the same content more than once or leave the original directory tree.

### Input selection

- An explicit `-` target searches stdin.
- With no target, redirected stdin supplies search content.
- With no target and terminal stdin, the current directory is searched.
- If `-f -` reserves stdin for patterns, an omitted target selects the current directory.
- `--files` defaults to the current directory, regardless of redirected stdin.
- Use `./-` for a filesystem entry named `-`.

Stdin can supply search content, patterns, or a path list, but only one of these per invocation. Search output labels stdin as `(standard input)`.

Path lists accept LF, CRLF, and an unterminated final line. Empty lines are skipped. Spaces, hash characters, duplicates, and list order are preserved. Relative paths resolve from the current working directory. A listed `-` is a filename. An empty list supplies no targets.

Path lists preserve native filename bytes on Unix and require UTF-8 on Windows. NUL bytes are rejected. Lists are loaded before discovery and output. A list-source error stops the operation; target errors allow later roots to continue.

`--files-from` conflicts with a positional target. Enumeration rejects patterns, matching flags, context, selection limits, `--text`, and competing output modes.

### Context, limits, and execution

| Option | Behavior |
|---|---|
| `-A, --after-context NUM` | Include following context lines. |
| `-B, --before-context NUM` | Include preceding context lines. |
| `-C, --context NUM` | Set both context counts. |
| `-m, --max-count NUM` | Limit selected lines per file. |
| `--max-results NUM` | Limit selected lines across the search. |
| `-q, --quiet` | Suppress results and stop at the first qualifying result. |
| `-j, --threads NUM` | Request a positive worker count. Default: `1`. |
| `--sort none\|path` | Stream discovery order or collect and sort native paths. Default: `none`. |

Explicit `-A` and `-B` values override their side of `-C`, regardless of argument order. Adjacent and overlapping context windows merge. Separate groups use `--` separators.

Limits count selected lines, including inverted selections. Context and repeated occurrences do not consume additional selections. Omitted limits are unlimited. Zero limits select nothing. Pattern and path-list validation still occurs with a zero global limit.

Line output can finish requested after-context after a limit is reached. Count and match-only modes stop without trailing context.

Parallel execution applies to filesystem search when quiet mode and a finite global limit are absent. Stdin, enumeration, quiet mode, and finite-global-limit searches use sequential execution.

The worker pool uses bounded queues and limits active files. The coordinator emits complete file groups in candidate order. A slow early file can delay later groups.

`--sort path` collects candidates before processing. Sorting uses native path ordering and preserves duplicates. Discovery errors encountered during collection are reported before search output. Sorting requires memory proportional to the candidate list.

### Output

| Option | Behavior |
|---|---|
| `-n, --line-number` | Include one-based line numbers. |
| `--column` | Include one-based byte columns and imply line numbers. |
| `-H, --with-filename` | Include filename prefixes. This is the default. |
| `-h, --no-filename` | Omit filename prefixes from line, match-only, and count output. |
| `--heading` | Group visible line and match-only records beneath file paths. |
| `--color auto\|always\|never` | Control match highlighting. Default: `auto`. |
| `-0, --null` | Write NUL after emitted filenames. |
| `-l, --files-with-matches` | Print paths containing selected lines. |
| `--files-without-match` | Print paths containing no selected lines. |
| `-c, --count` | Print selected-line counts, including zero. |
| `--count-matches` | Print match-span counts, including zero. |
| `-o, --only-matching` | Print nonempty spans as separate records. |
| `--help` | Print help and exit successfully. |
| `-V, --version` | Print the version and exit successfully. |

Default records use `path:text`. With `-n`, selected lines use `path:line:text`, and context uses `path-line-text`. Line numbers restart per file. Filenames are included even for one explicit file.

Columns measure UTF-8 bytes, not characters or terminal cells. Context and inverted selections without spans use column one. `-h` removes only the filename field. File-list and enumeration modes always print paths.

File-list, count, and match-only modes are mutually exclusive. Match-only and occurrence-count modes reject inversion. Context options are ignored in count, file-list, and match-only modes.

Counts are emitted only for successfully completed evaluations. An interrupted, binary-stopped, or failed read does not emit a partial count. Earlier completed records remain.

Zero-width matches affect selection, limits, and occurrence counts. They produce no match-only records. A successful search can therefore print no records.

`--files-without-match` requires successful completion without selected lines. Empty files qualify. A read failure or binary stop does not qualify.

Color `auto` uses stdout terminal status. Environment-variable color preferences are not consulted. Only matched text is highlighted; prefixes, headings, and context remain plain. Original input control characters are not escaped.

`-H` conflicts with `-h`. Headings conflict with `-h` and NUL output. `-h` controls filenames; help uses `--help`.

NUL output preserves native filename bytes on Unix. Windows rejects unpaired UTF-16 surrogates for this output mode. NUL-delimited filenames are not a structured format for arbitrary search text.

### Exit status and failures

| Status | Meaning |
|---|---|
| `0` | A selected line, qualifying file, or candidate exists for the requested mode; help/version also use zero. |
| `1` | No qualifying result exists. |
| `2` | A usage or operational error occurred, or cancellation was observed. |

Diagnostics go to stderr. File and discovery errors preserve earlier results and allow available files to continue. An earlier operational error retains status two, including after quiet stopping.

A stdout error classified as `BrokenPipe` stops processing without a broken-pipe diagnostic. It returns zero unless an earlier error requires two. Other output errors and diagnostic failures return two.

The first Ctrl+C requests cooperative cancellation. Observed cancellation prints `mini-grep: operation cancelled` and returns two. Active blocking I/O or regex work can delay observation. A second Ctrl+C forces termination and can skip cleanup, buffered output, and diagnostics.

## Examples

Search recursively with context and a file-type filter.

Terminal command (run from repository root)

```powershell
cargo run -- -n -C 1 --type rust "Result<" src
```

Combine patterns and print selected-line counts.

Terminal command (run from repository root)

```powershell
cargo run -- -c -e Rust -e design tests/fixtures/text/demo.txt
```

Search stdin explicitly.

Terminal command (run from repository root)

```powershell
"Rust", "Go" | cargo run -- -n Rust -
```

Read target paths from stdin.

Terminal command (run from repository root)

```powershell
"tests/fixtures/text/demo.txt" | cargo run -- -n Rust --files-from -
```

Enumerate Rust files without reading their contents.

Terminal command (run from repository root)

```powershell
cargo run -- --files --type rust .
```

Search with two workers and sorted file groups.

Terminal command (run from repository root)

```powershell
cargo run -- -j2 --sort path --heading -n Rust tests/fixtures/text
```

## Architecture

Data flows from CLI parsing through validated configuration, discovery, input, matching, execution, and output. The application coordinates these components. Pure matching performs no filesystem or terminal I/O.

| Component | Responsibility |
|---|---|
| `src/main.rs` | Start the application, report errors, and select exit status. |
| `src/app.rs` | Compose inputs, matcher, executors, diagnostics, and output. |
| `src/cli/` | Define clap syntax and validated settings. |
| `src/discovery/` | Traverse paths and apply ignore, glob, type, depth, and size rules. |
| `src/input/` | Read files, stdin, patterns, and path lists; classify binary content. |
| `src/search/` | Prepare patterns, select lines, manage context, and define byte spans. |
| `src/execution/` | Process inputs, coordinate workers, sort candidates, and handle cancellation. |
| `src/output/` | Render records, headings, filename delimiters, color, and diagnostics. |
| `tests/` | Exercise the library, executable, input, discovery, and failure behavior. |
| `benches/` | Measure matching, readers, synthetic files, and source snapshots. |
| `scripts/` | Generate memory fixtures and measure Windows working sets. |

Files use a 64 KiB read buffer. Execution returns discarded line allocations to the reader for reuse. Selected output and context retain owned text. Workers reuse the sequential processor and send owned events to the coordinator.

The public `mini_grep::search::search(pattern, text)` function performs case-sensitive literal substring search. It returns `Vec<LineMatch<'_>>` with borrowed text and validated `MatchSpan` byte ranges. Keep the input alive while using results. LF and CRLF line endings are excluded from results. Empty input returns no results. An empty pattern matches each line returned by `str::lines()`. This library interface is separate from the CLI's prepared regex matching.

### Technology stack

| Dependency | Role |
|---|---|
| Rust and Cargo | Application, package management, and builds. |
| `clap` 4 | CLI parsing, help, and version output. |
| `ignore` 0.4 | Discovery, local ignore rules, globs, and built-in file types. |
| `regex` 1 and `regex-syntax` 0.8 | Prepared matching and syntax-preserving scope construction. |
| `crossbeam-channel` 0.5 | Bounded worker and event queues. |
| `ctrlc` 3 | Cancellation signal handling. |
| `anstream` 1 | Terminal-aware styled output. |
| `criterion` 0.8.2 | Development-only statistical benchmarks. |

## Configuration

Runtime behavior is controlled by CLI options. Configuration files and configuration subcommands are not implemented.

The CLI comparison benchmarks use `GREP_BENCH_EXE` as an optional GNU grep executable path. Without it, they resolve `grep` through `PATH`. The benchmark executables also resolve `rg` through `PATH`. These are benchmark setup requirements, not application configuration.

## Testing

Run the required checks.

Terminal command (run from repository root)

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-features
```

Tests cover matching, byte spans, CLI grammar, context, limits, output modes, filtering, errors, cancellation, ordering, and bounded execution. Integration tests cover stdin, partial results, and broken output pipes.

The latest local Windows run completed all three commands successfully. Unix-only cases exist but have not been verified on Unix. Windows symlink tests require Developer Mode or suitable symlink privileges. See [Rust's Windows symlink requirements](https://doc.rust-lang.org/std/os/windows/fs/fn.symlink_file.html#limitations).

## Performance

Final baseline: `final-suite`. All **65 benchmark cases** completed. Formatting, Clippy, and all **443 Rust tests** passed.

Times below are point estimates. Lower is better. CLI timings include process startup, discovery, reading, matching, and count output to null streams. Inputs were checked before timing.

Comparisons used ripgrep 15.2.0 and GNU grep 3.0 from Git for Windows. Workloads were timed sequentially in separate phases with warm inputs. Results are specific to these workloads.

### Synthetic files

Each layout contains **17,285,120 bytes and 524,288 ASCII lines**. The directory layout contains 64 files. Sparse literal and regex cases select 8,192 lines; dense literal selects every line.

All times are milliseconds.

| Workload | Layout | Workers | mini-grep | ripgrep | GNU grep |
|---|---|---:|---:|---:|---:|
| Sparse literal | Single file | 1 | 69.655 | 28.016 | 44.495 |
| Sparse literal | 64 files | 1 | 80.765 | 39.018 | 79.218 |
| Sparse literal | 64 files | 2 | 64.988 | 32.674 | N/A |
| Sparse literal | 64 files | 4 | 49.215 | 30.232 | N/A |
| No match | Single file | 1 | 74.140 | 26.343 | 71.965 |
| No match | 64 files | 1 | 76.701 | 34.889 | 100.410 |
| No match | 64 files | 2 | 63.434 | 28.829 | N/A |
| No match | 64 files | 4 | 46.999 | 25.593 | N/A |
| Dense literal | Single file | 1 | 61.330 | 54.486 | 44.590 |
| Dense literal | 64 files | 1 | 71.572 | 65.002 | 79.250 |
| Dense literal | 64 files | 2 | 59.717 | 44.177 | N/A |
| Dense literal | 64 files | 4 | 47.009 | 33.711 | N/A |
| Regex | Single file | 1 | 69.143 | 29.225 | 45.339 |
| Regex | 64 files | 1 | 80.518 | 38.721 | 84.369 |
| Regex | 64 files | 2 | 65.977 | 32.144 | N/A |
| Regex | 64 files | 4 | 47.410 | 28.252 | N/A |

GNU grep runs single-worker cases only. Its synthetic regex uses an equivalent POSIX expression for these ASCII inputs. Every case checks exact per-file counts, exit status, and stderr.

### Source tree

The snapshot contains **41 Rust files and 425,446 bytes**. Literal search uses `Rust`; regex search uses `\bRust\b`.

All times are milliseconds.

| Mode | Workers | mini-grep | ripgrep | GNU grep |
|---|---:|---:|---:|---:|
| Literal | 1 | 31.317 | 30.185 | 73.200 |
| Literal | 2 | 28.232 | 27.680 | N/A |
| Literal | 4 | 25.427 | 25.917 | N/A |
| Regex | 1 | 31.870 | 31.152 | N/A |
| Regex | 2 | 28.011 | 27.206 | N/A |
| Regex | 4 | 25.118 | 25.802 | N/A |

GNU grep is included only in literal source-tree search. Equivalent Unicode word-boundary behavior has not been validated for its regex mode. Changing Rust files changes this benchmark's dataset.

Ripgrep had the lowest estimates in most CLI comparisons. GNU grep led the dense single-file case. Mini-grep had slightly lower four-worker source-tree estimates. These results do not establish a general tool ranking.

### In-memory literal search

These results include search, result allocation, and result destruction. They exclude CLI startup and filesystem input. One mebibyte (MiB) is 1,048,576 bytes.

| Text | Lines | Input bytes | Time | Throughput |
|---|---:|---:|---:|---:|
| ASCII | 1,024 | 33,760 | 83.420 microseconds | 385.95 MiB/s |
| ASCII | 65,536 | 2,160,640 | 5.8174 ms | 354.20 MiB/s |
| Unicode | 1,024 | 86,560 | 239.307 microseconds | 344.95 MiB/s |
| Unicode | 65,536 | 5,539,840 | 12.3277 ms | 428.56 MiB/s |

ASCII and Unicode fixtures use different text and patterns. Their timings do not isolate Unicode overhead.

### Reader experiment

Each input contains 17,285,120 bytes. Both counters use a 64 KiB reader over in-memory input. All times are milliseconds.

| Input | Whole-line buffer | Chunked prototype |
|---|---:|---:|
| Short lines | 35.411 | 37.090 |
| Long line, match at start | 29.736 | 42.097 |
| Long line, no match | 40.252 | 30.162 |
| Long line, match at end | 33.559 | 30.592 |

The chunked counter remains experimental. It omits production regex, UTF-8 validation, binary policy, context, cancellation, and rendering.

### Peak memory

Latest developer-reported measurements use five fresh single-worker processes per input. They were not rerun with `final-suite`.

| Input | Input bytes | Median peak working set |
|---|---:|---:|
| Short lines | 17,285,120 | 5.750 MiB |
| Eightfold short-line input | 138,280,960 | 5.766 MiB |
| One long line | 17,285,120 | 38.270 MiB |

Working set includes private and shared resident pages. Long lines increase memory because the reader buffers a complete line.

The heap trace recorded **1,052 allocation records** and approximately **0.323 MB**, as displayed by WPA. These whole-process figures include startup and dependencies. They are distinct from peak working set and Rust-only allocation counts.

## Design decisions and limitations

- **Buffered input remains the default.** The CLI processes complete UTF-8 lines. LF and CRLF endings are removed.
- **Binary detection precedes decoding.** A NUL byte stops the input without an operational error. Earlier records remain. `--text` disables this heuristic.
- **Memory depends on line length.** Long lines, context buffers, pattern sources, path lists, sorted paths, and queued events can increase memory use.
- **Input encoding is limited.** There is no transcoding. `--text` does not enable UTF-16, legacy encodings, or arbitrary-byte search.
- **Path lists are line-based.** They cannot represent filenames containing newlines. `-0` changes output only.
- **Filename display can be lossy.** NUL filename output preserves native Unix bytes; ordinary display can replace invalid Unicode.
- **Filesystem reads are not snapshots.** Ignore audits, metadata checks, and content reads can observe different states.
- **Control characters remain intact.** Search text is not escaped before terminal output.
- **Mmap is deferred.** Neither `--mmap` nor `--no-mmap` is implemented. No mapping dependency was added.
- **Benchmark coverage is limited.** Unicode CLI workloads, larger repositories, broader regex patterns, exact allocated-byte totals, and multi-worker memory need further measurements.

File-backed mappings introduce safety requirements when other writers can modify the mapped file. A prior metadata check does not enforce stability during use. See [memmap2's safety documentation](https://docs.rs/memmap2/latest/memmap2/struct.MmapOptions.html). No mapping benefit has been established for this application.

## Planned features

- JSON Lines output (one JavaScript Object Notation record per line) and summary statistics.
- Configuration files and shell completions.
- Cross-platform CI and release binaries.
- Broader performance and memory measurements.
- Optional terminal user interface and local indexing.
