# CLI and compatibility contract

## Search invocation

Search accepts:

- `mini-grep [OPTIONS] <PATTERN> [PATH]`
- `mini-grep [OPTIONS] (-e <PATTERN> | -f <FILE>)... [PATH]`
- `mini-grep --files [OPTIONS] [PATH]`

Utility commands must appear as the first argument:

- `mini-grep config show [--no-config]`
- `mini-grep config path`
- `mini-grep completions <SHELL>`

Supported completion shells are bash, zsh, fish, and powershell.

Use a leading separator to search for a utility name:
mini-grep -- config show searches for config in the file show.

-h hides filenames. --help displays help.

## Exit status

| Status | Meaning |
|---|---|
| 0 | Search selected a result, or a utility completed successfully |
| 1 | Search selected no results |
| 2 | Usage or operational error |

Recoverable file errors preserve successful output and return status 2.

Quiet mode stops early. Errors already encountered take precedence over
a selected result. It does not report errors in inputs that were never
processed.

A closed stdout pipe is handled without a panic. It returns status 0
unless an earlier operational error requires status 2. Diagnostic
write failures return status 2.

## JSON Lines

--json writes compact UTF-8 JSON objects to stdout. Each completed
record ends with LF. Diagnostics remain plain text on stderr.

Every search record has:

- schema_version: integer, currently 1.
- type: string identifying the record.

Per-input records also have source. A run-wide summary has no source.

JSON object field order is not significant.

### Source identity

A file source has kind: "file" and an encoded path.
Stdin has kind: "stdin" and no path.

Encoded paths contain encoding and value:

| Encoding | Value |
|---|---|
| utf8 | String |
| unix_bytes | Array of byte values |
| windows_utf16 | Array of UTF-16 code units |
| platform_bytes | Array of platform-specific byte values |

Native encodings preserve paths that cannot be represented as UTF-8.
They do not promise portability between operating systems.

A file named (standard input) remains a file source.

### Record types

| Type | Additional fields |
|---|---|
| begin | None |
| line | line_number, selected, text, spans |
| match | line_number, text, start, end |
| count | count, unit |
| path | None |
| group_break | None |
| summary | Statistics fields described below |

begin marks the start of input processing. It does not establish that
the input was fully read, contained a match, or completed successfully.
There is no per-input end record.

Line numbers are one-based integers. Line text excludes its line ending.

A line record contains the full line. Each span contains start and
end. Offsets are zero-based UTF-8 byte offsets with an exclusive end.

Context lines have selected: false and empty spans.

A match record contains only the matched substring. Its offsets refer
to the original full line. Match-only output omits zero-width matches.

Count values are decimal strings. Units are selected_lines or
matches.

--files --json emits path records without searching file contents.

### Presentation and failures

JSON ignores terminal presentation settings: line numbers, columns,
filename visibility, headings, color, and NUL filename separators.
Existing conflicts between CLI options still apply.

Quiet mode suppresses every JSON record.

Completed records remain valid when a later input fails. An output
failure can leave an incomplete final record. Consumers must check the
exit status and must not treat begin as evidence of completion.

## Statistics

--stats conflicts with quiet mode and file enumeration.

Human-readable statistics go to stderr. With --json, statistics are
one final summary record on stdout.

Counters describe work performed, including work before a recoverable
error or early exit:

| Field | Meaning |
|---|---|
| inputs_started | Input attempts, including failed file opens |
| inputs_with_selected_lines | Inputs with at least one selected line |
| selected_lines | Selected lines processed before stopping |
| matches | Match spans on selected lines |
| errors | Recoverable discovery, open, and read errors |

Stdin counts as one input. Context-only lines do not increase selection
counters. Inverted selections have no match spans.

Discovery failures do not count as started inputs. Zero limits skip
input processing. An input that fails after producing selections
retains those measurements.

Counters saturate at u128::MAX.

JSON counters and elapsed_nanoseconds are decimal strings.
The terminal summary reports elapsed_seconds with six decimal places.

Elapsed time starts after CLI parsing and configuration validation.
It is captured before writing the summary and performing final flushes.

Fatal setup errors, cancellation, and output failures can terminate
without a summary.

## Local configuration

The optional file is .mini-grep.json in the current working directory.
Parent directories and user-wide locations are not searched.

Supported settings are:

| Setting | Values | Built-in default |
|---|---|---|
| threads | Positive integer fitting usize | 1 |
| color | auto, always, never | auto |
| sort | none, path | none |

Precedence is:

1. Explicit CLI option.
2. Local configuration value.
3. Built-in default.

Explicit CLI values override the file even when they equal built-in
defaults. Omitted or null file settings leave built-in defaults in place.

A missing file is allowed. Invalid UTF-8, malformed JSON, unknown or
duplicate fields, invalid setting values, and other read failures
return status 2.

--no-config skips loading the file.

config show prints resolved defaults as one JSON document.
config show --no-config prints built-in defaults.

config path prints the expected absolute path as an encoded-path JSON
document. It does not load the file.

Inspection commands do not create configuration files. Help, version,
and completion generation do not load configuration.

## Compatibility rules

JSON schema version 1 freezes existing field names, types, units,
offset meanings, and record semantics.

New fields and record types may be added without changing the schema
version. Consumers must ignore unknown fields and skip unknown record
types.

Removing or renaming fields, changing field types, or changing existing
meanings requires a new schema version.

Configuration names, value meanings, and precedence must not change
silently. New settings must preserve existing configuration behavior.
Older binaries can reject settings introduced by newer versions.

Existing CLI flag meanings, utility names, and exit-status policies
must not change silently. Intentional breaking changes require release
notes and a documented migration path.

Completion tests verify script generation and expected content.
Interactive behavior in each target shell requires separate validation.
