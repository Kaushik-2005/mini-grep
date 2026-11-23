#[cfg(unix)]
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    directory: PathBuf,
    path: PathBuf,
}

impl Fixture {
    fn new(contents: &[u8]) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after the Unix epoch")
            .as_nanos();

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "mini-grep-test-{}-{timestamp}-{id}",
            std::process::id()
        ));

        fs::create_dir(&directory).expect("temporary directory must be created");

        let fixture = Self {
            path: directory.join("sample file.txt"),
            directory,
        };

        fs::write(&fixture.path, contents).expect("fixture must be written");
        fixture
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

#[cfg(unix)]
fn rename_non_utf8_fixture(fixture: &mut Fixture, renamed: PathBuf) -> bool {
    match fs::rename(&fixture.path, &renamed) {
        Ok(()) => {
            fixture.path = renamed;
            true
        }
        Err(error) => {
            #[cfg(target_os = "macos")]
            {
                const EILSEQ: i32 = 92;

                if error.raw_os_error() == Some(EILSEQ) {
                    eprintln!(
                        "Skipping filesystem-dependent assertion: \
                        this filesystem rejects non-UTF-8 filenames"
                    );
                    return false;
                }
            }

            panic!("cannot rename fixture to '{}': {error}", renamed.display());
        }
    }
}

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mini-grep"));
    command.current_dir(env!("CARGO_MANIFEST_DIR"));
    command
}

fn search(pattern: &str, path: &Path) -> Output {
    command()
        .arg("-n")
        .arg(pattern)
        .arg(path)
        .output()
        .expect("executable must start")
}

fn assert_operational_error(output: &Output, path: &Path) {
    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8_lossy(&output.stderr);
    let displayed_path = path.display().to_string();

    assert!(stderr.starts_with("mini-grep: "), "{stderr}");
    assert!(stderr.contains(displayed_path.as_str()), "{stderr}");
    assert!(!stderr.contains("panicked at"), "{stderr}");
}

fn assert_selected_lines(output: Output, path: &Path, lines: &[(usize, &str)]) {
    let expected_code = if lines.is_empty() { 1 } else { 0 };

    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected: String = lines
        .iter()
        .map(|(number, text)| format!("{}:{number}:{text}\n", path.display()))
        .collect();

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn searches_unicode_crlf_content_in_a_path_with_spaces() {
    let fixture = Fixture::new("first\r\nनमस्ते Rust Rust\r\nlast".as_bytes());
    let output = search("Rust", &fixture.path);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:2:नमस्ते Rust Rust\n", fixture.path.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn no_match_returns_one_without_output() {
    let fixture = Fixture::new(b"Rust\n");
    let output = search("rust", &fixture.path);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn empty_file_returns_one_even_for_an_empty_pattern() {
    let fixture = Fixture::new(b"");
    let output = search("", &fixture.path);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn empty_pattern_matches_blank_lines_without_an_extra_trailing_line() {
    let fixture = Fixture::new(b"first\n\nlast\n");
    let output = search("", &fixture.path);
    let path = fixture.path.display();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{path}:1:first\n{path}:2:\n{path}:3:last\n");
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn invalid_argument_counts_return_two_and_usage() {
    let cases: &[&[&str]] = &[&[], &["Rust", "file.txt", "extra.txt"]];

    for args in cases {
        let output = command()
            .args(*args)
            .output()
            .expect("executable must start");

        assert_eq!(output.status.code(), Some(2), "arguments: {args:?}");
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("error:"), "{stderr}");
        assert!(stderr.contains("Usage:"), "{stderr}");
    }
}

#[test]
fn missing_file_returns_two_with_path_context() {
    let fixture = Fixture::new(b"");
    let missing = fixture.directory.join("missing.txt");
    let output = search("Rust", &missing);

    assert_operational_error(&output, &missing);
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot discover files"), "{stderr}");
}

#[test]
fn directory_input_searches_its_files() {
    let fixture = Fixture::new(b"Rust\n");
    let output = search("Rust", &fixture.directory);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:1:Rust\n", fixture.path.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn invalid_utf8_returns_two_with_read_context() {
    let fixture = Fixture::new(&[0xff, b'\n']);
    let output = search("Rust", &fixture.path);

    assert_operational_error(&output, &fixture.path);
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot read"), "{stderr}");
}

#[test]
fn read_error_after_a_match_preserves_output_and_returns_two() {
    let fixture = Fixture::new(b"Rust\n\xff\n");
    let output = search("Rust", &fixture.path);

    assert_operational_error(&output, &fixture.path);

    let expected = format!("{}:1:Rust\n", fixture.path.display());
    assert_eq!(output.stdout, expected.as_bytes());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot read"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn accepts_a_non_utf8_file_path() {
    use std::os::unix::ffi::OsStrExt;

    let mut fixture = Fixture::new(b"Rust\n");
    let renamed = fixture
        .directory
        .join(OsStr::from_bytes(b"sample-\xff.txt"));

    if !rename_non_utf8_fixture(&mut fixture, renamed) {
        return;
    }

    let output = search("Rust", &fixture.path);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:1:Rust\n", fixture.path.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[cfg(unix)]
#[test]
fn rejects_a_non_utf8_pattern() {
    use std::os::unix::ffi::OsStrExt;

    let fixture = Fixture::new(b"Rust\n");

    let output = command()
        .arg(OsStr::from_bytes(b"\xff"))
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("error:"), "{stderr}");
    assert!(stderr.contains("UTF-8"), "{stderr}");
}

#[cfg(windows)]
#[test]
fn exclusively_locked_file_returns_two_without_panicking() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    let fixture = Fixture::new(b"Rust\n");

    let lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&fixture.path)
        .expect("exclusive file handle must open");

    let output = search("Rust", &fixture.path);

    drop(lock);

    assert_operational_error(&output, &fixture.path);
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot open"), "{stderr}");
}

#[test]
fn default_output_omits_line_numbers() {
    let fixture = Fixture::new(b"first\nRust\n");

    let output = command()
        .arg("Rust")
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:Rust\n", fixture.path.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn search_flags_work_in_short_long_and_combined_forms() {
    let fixture = Fixture::new(b"Rust\nrust\nGo\n");
    let path = fixture.path.display();

    let cases = [
        ("-i", format!("{path}:Rust\n{path}:rust\n")),
        ("--ignore-case", format!("{path}:Rust\n{path}:rust\n")),
        ("-n", format!("{path}:1:Rust\n")),
        ("--line-number", format!("{path}:1:Rust\n")),
        ("-v", format!("{path}:rust\n{path}:Go\n")),
        ("--invert-match", format!("{path}:rust\n{path}:Go\n")),
        ("-iFnv", format!("{path}:3:Go\n")),
    ];

    for (flag, expected) in cases {
        let output = command()
            .args([flag, "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(output.status.code(), Some(0), "flag: {flag}");
        assert!(output.stderr.is_empty(), "flag: {flag}");
        assert_eq!(output.stdout, expected.as_bytes(), "flag: {flag}");
    }
}

#[test]
fn ignores_unicode_case_without_changing_output_text() {
    let fixture = Fixture::new("Un café\n".as_bytes());

    let output = command()
        .args(["-i", "CAFÉ"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:Un café\n", fixture.path.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn fixed_string_modes_treat_metacharacters_literally() {
    let fixture = Fixture::new(b"plain\ndot . here\n");
    let cases: &[&[&str]] = &[&["-F"], &["--fixed-strings"]];

    for flags in cases {
        let output = command()
            .args(*flags)
            .arg(".")
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());

        let expected = format!("{}:dot . here\n", fixture.path.display());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn inverted_empty_pattern_selects_no_lines() {
    let fixture = Fixture::new(b"Rust\n\nGo\n");

    for flag in ["-v", "--invert-match", "-iv"] {
        let output = command()
            .args([flag, ""])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(output.status.code(), Some(1), "flag: {flag}");
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn help_and_version_succeed_without_search_arguments() {
    let help = command()
        .arg("--help")
        .output()
        .expect("executable must start");

    assert_eq!(help.status.code(), Some(0));
    assert!(help.stderr.is_empty());

    let stdout = String::from_utf8_lossy(&help.stdout);

    for expected in [
        "Usage:",
        "<PATTERN>",
        "[PATH]",
        "--ignore-case",
        "--fixed-strings",
        "--line-number",
        "--invert-match",
        "--help",
        "--version",
    ] {
        assert!(stdout.contains(expected), "missing {expected}: {stdout}");
    }

    for flag in ["--version", "-V"] {
        let version = command().arg(flag).output().expect("executable must start");

        assert_eq!(version.status.code(), Some(0));
        assert!(version.stderr.is_empty());

        let expected = format!("mini-grep {}\n", env!("CARGO_PKG_VERSION"));
        assert_eq!(version.stdout, expected.as_bytes());
    }
}

#[test]
fn rejects_unknown_and_repeated_flags() {
    let fixture = Fixture::new(b"Rust\n");

    for flag in ["--unknown", "--line-numbers", "-ii"] {
        let output = command()
            .args([flag, "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(output.status.code(), Some(2), "flag: {flag}");
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("error:"), "{stderr}");
        assert!(stderr.contains("Usage:"), "{stderr}");
    }
}

#[test]
fn separator_allows_a_pattern_beginning_with_a_hyphen() {
    let fixture = Fixture::new(b"plain\n-needle\n");

    let output = command()
        .args(["--", "-needle"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:-needle\n", fixture.path.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn empty_path_returns_a_usage_error() {
    let output = command()
        .args(["Rust", ""])
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("error:"), "{stderr}");
    assert!(stderr.contains("[PATH]"), "{stderr}");
}

#[test]
fn default_mode_interprets_regex_syntax() {
    let fixture = Fixture::new(b"plain\ndot . here\n");
    let output = search(".", &fixture.path);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let path = fixture.path.display();
    let expected = format!("{path}:1:plain\n{path}:2:dot . here\n");

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn smart_case_short_and_long_flags_select_case_behavior() {
    let fixture = Fixture::new(b"rust\nRUST\nGo\n");

    let cases: &[(&str, &[(usize, &str)])] = &[
        ("rust", &[(1, "rust"), (2, "RUST")]),
        ("RUST", &[(2, "RUST")]),
    ];

    for flag in ["-S", "--smart-case"] {
        for &(pattern, expected) in cases {
            let output = command()
                .args(["-n", flag, pattern])
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_selected_lines(output, &fixture.path, expected);
        }
    }
}

#[test]
fn whole_word_mode_handles_words_and_punctuation() {
    let fixture = Fixture::new(b"cat\nscatter\n_cat\ncat_\n(cat)\ncat cat\n@\nx@y\n");

    let cases: &[(&str, &[(usize, &str)])] = &[
        ("cat", &[(1, "cat"), (5, "(cat)"), (6, "cat cat")]),
        ("@", &[(7, "@")]),
    ];

    for flag in ["-w", "--word-regexp"] {
        for &(pattern, expected) in cases {
            let output = command()
                .args(["-n", flag, pattern])
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_selected_lines(output, &fixture.path, expected);
        }
    }
}

#[test]
fn whole_line_mode_supports_regex_alternatives_and_literal_mode() {
    let fixture = Fixture::new(b"a\nab\nzab\nabz\na|ab\n");

    for flag in ["-x", "--line-regexp"] {
        let output = command()
            .args(["-n", flag, "a|ab"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_selected_lines(output, &fixture.path, &[(1, "a"), (2, "ab")]);
    }

    let literal = command()
        .args(["-n", "-F", "-x", "a|ab"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(literal, &fixture.path, &[(5, "a|ab")]);
}

#[test]
fn repeated_explicit_patterns_print_each_selected_line_once() {
    let fixture = Fixture::new(b"Rust\nGo\nPython\nRust Go\n");

    for flag in ["-e", "--regexp"] {
        let output = command()
            .args(["-n", flag, "Rust", flag, "Go", flag, "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_selected_lines(
            output,
            &fixture.path,
            &[(1, "Rust"), (2, "Go"), (4, "Rust Go")],
        );
    }
}

#[test]
fn conflicting_matching_flags_are_usage_errors() {
    let fixture = Fixture::new(b"Rust\n");

    for flags in ["-iS", "-Si", "-wx", "-xw"] {
        let output = command()
            .args(["-n", flags, "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(output.status.code(), Some(2), "flags: {flags}");
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(stderr.starts_with("error:"), "{stderr}");
        assert!(stderr.contains("cannot be used"), "{stderr}");
    }
}

#[test]
fn explicit_pattern_equals_syntax_accepts_a_leading_hyphen() {
    let fixture = Fixture::new(b"-needle\nother\n");

    let output = command()
        .args(["-n", "--regexp=-needle"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(output, &fixture.path, &[(1, "-needle")]);
}

#[test]
fn unicode_simple_case_folding_preserves_text_without_normalizing() {
    let fixture = Fixture::new("Σ\nσ\nς\né\ne\u{301}\nStraße\n".as_bytes());

    let cases: &[(&str, &[(usize, &str)])] = &[
        ("σ", &[(1, "Σ"), (2, "σ"), (3, "ς")]),
        ("é", &[(4, "é")]),
        ("STRASSE", &[]),
    ];

    for &(pattern, expected) in cases {
        let output = command()
            .args(["-n", "-i", pattern])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_selected_lines(output, &fixture.path, expected);
    }
}

#[test]
fn combines_explicit_patterns_and_multiple_pattern_files() {
    let first = Fixture::new(b"Rust\r\n");
    let second = Fixture::new(b"Go\n");
    let target = Fixture::new(b"Rust\nGo\nPython\nRust Go\n");

    let output = command()
        .args(["-n", "-e", "Python", "-f"])
        .arg(&first.path)
        .arg("--file")
        .arg(&second.path)
        .arg(&target.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(
        output,
        &target.path,
        &[(1, "Rust"), (2, "Go"), (3, "Python"), (4, "Rust Go")],
    );
}

#[test]
fn smart_case_considers_pattern_file_contents() {
    let patterns = Fixture::new(b"GO\n");
    let target = Fixture::new(b"rust\nRUST\nGo\n");

    let output = command()
        .args(["-n", "-S", "-e", "rust", "-f"])
        .arg(&patterns.path)
        .arg(&target.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(output, &target.path, &[(1, "rust")]);
}

#[test]
fn empty_pattern_sources_have_distinct_selection_behavior() {
    let empty_file = Fixture::new(b"");
    let blank_pattern = Fixture::new(b"\n");
    let target = Fixture::new(b"Rust\n\nGo\n");

    let cases = [
        ("-f", empty_file.path.as_os_str(), false, false),
        ("-f", blank_pattern.path.as_os_str(), false, true),
        ("-f", empty_file.path.as_os_str(), true, true),
        ("-f", blank_pattern.path.as_os_str(), true, false),
        ("-e", std::ffi::OsStr::new(""), false, true),
        ("-e", std::ffi::OsStr::new(""), true, false),
    ];

    for (flag, value, invert, selects_all) in cases {
        let mut process = command();
        process.arg("-n");

        if invert {
            process.arg("-v");
        }

        let output = process
            .arg(flag)
            .arg(value)
            .arg(&target.path)
            .output()
            .expect("executable must start");

        let expected: &[(usize, &str)] = if selects_all {
            &[(1, "Rust"), (2, ""), (3, "Go")]
        } else {
            &[]
        };

        assert_selected_lines(output, &target.path, expected);
    }
}

#[test]
fn pattern_file_errors_are_reported_before_search_output() {
    let invalid = Fixture::new(b"Rust\n\xff\n");
    let target = Fixture::new(b"Rust\n");
    let missing = invalid.directory.join("missing.patterns");

    let cases = [
        (invalid.path.as_path(), "cannot read"),
        (missing.as_path(), "cannot open"),
    ];

    for (pattern_path, operation) in cases {
        let output = command()
            .args(["-n", "-e", "Rust", "-f"])
            .arg(pattern_path)
            .arg(&target.path)
            .output()
            .expect("executable must start");

        assert_operational_error(&output, pattern_path);
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(operation), "{stderr}");
    }
}

#[test]
fn invalid_patterns_fail_before_discovery_or_search_output() {
    let target = Fixture::new(b"Rust\n");
    let patterns = Fixture::new(b"Rust\n[\n");
    let missing_target = target.directory.join("missing-target.txt");

    let direct = command()
        .args(["-n", "["])
        .arg(&missing_target)
        .output()
        .expect("executable must start");

    let from_file = command()
        .args(["-n", "-f"])
        .arg(&patterns.path)
        .arg(&target.path)
        .output()
        .expect("executable must start");

    for output in [direct, from_file] {
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(stderr.contains("invalid search pattern"), "{stderr}");
        assert!(!stderr.contains("cannot discover files"), "{stderr}");
        assert!(!stderr.contains("panicked at"), "{stderr}");
    }
}

#[test]
fn explicit_pattern_sources_reject_invalid_argument_combinations() {
    let target = Fixture::new(b"Rust\n");

    let extra_pattern = command()
        .args(["-e", "Rust", "Go"])
        .arg(&target.path)
        .output()
        .expect("executable must start");

    let stdin_patterns = command()
        .args(["-f", "-", "-"])
        .output()
        .expect("executable must start");

    let stderr = String::from_utf8_lossy(&stdin_patterns.stderr);
    assert!(stderr.contains("stdin"), "{stderr}");

    for output in [extra_pattern, stdin_patterns] {
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("error:"), "{stderr}");
        assert!(stderr.contains("Usage:"), "{stderr}");
    }
}

#[test]
fn context_output_groups_lines_and_marks_selected_lines() {
    let fixture = Fixture::new(b"before\nRust\nafter\nomitted\nbefore again\nRust\nafter again\n");

    for flag in ["-C", "--context"] {
        let output = command()
            .args(["-n", flag, "1", "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let path = fixture.path.display();
        let expected = format!(
            "{path}-1-before\n\
            {path}:2:Rust\n\
            {path}-3-after\n\
            --\n\
            {path}-5-before again\n\
            {path}:6:Rust\n\
            {path}-7-after again\n"
        );

        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn context_side_overrides_work_in_either_argument_order() {
    let fixture = Fixture::new(b"first\nbefore\nRust\nafter\nlast\n");
    let path = fixture.path.display();

    let cases: &[(&[&str], &str)] = &[
        (&["-C", "2", "-B", "0", "-A", "1"], "after"),
        (&["-A", "1", "-B", "0", "-C", "2"], "after"),
        (&["-C", "2", "-A", "0", "-B", "1"], "before"),
        (&["-B", "1", "-A", "0", "-C", "2"], "before"),
    ];

    for &(flags, side) in cases {
        let output = command()
            .arg("-n")
            .args(flags)
            .arg("Rust")
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(output.status.code(), Some(0), "flags: {flags:?}");
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let expected = match side {
            "after" => format!("{path}:3:Rust\n{path}-4-after\n"),
            _ => format!("{path}-2-before\n{path}:3:Rust\n"),
        };

        assert_eq!(output.stdout, expected.as_bytes(), "flags: {flags:?}");
    }
}

#[test]
fn inverted_selection_receives_context_without_selecting_context_lines() {
    let fixture = Fixture::new(b"Rust\nGo\nRust\nRust\n");

    let output = command()
        .args(["-n", "-v", "-C", "1", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let path = fixture.path.display();
    let expected = format!(
        "{path}-1-Rust\n\
        {path}:2:Go\n\
        {path}-3-Rust\n"
    );

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn context_without_selected_lines_produces_no_output() {
    for contents in [b"Go\nPython\n".as_slice(), b"".as_slice()] {
        let fixture = Fixture::new(contents);

        for flags in [["-A", "3"], ["-B", "3"], ["-C", "3"]] {
            let output = command()
                .args(flags)
                .arg("Rust")
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(
                output.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[test]
fn read_failure_preserves_emitted_context_and_returns_two() {
    let fixture = Fixture::new(b"before\nRust\nafter\n\xff\n");

    let output = command()
        .args(["-n", "-C", "2", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_operational_error(&output, &fixture.path);

    let path = fixture.path.display();
    let expected = format!(
        "{path}-1-before\n\
         {path}:2:Rust\n\
         {path}-3-after\n"
    );

    assert_eq!(output.stdout, expected.as_bytes());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot read"), "{stderr}");
}

#[test]
fn per_file_limit_counts_selected_lines_instead_of_occurrences() {
    let fixture = Fixture::new(b"Rust Rust\nGo\nRust\nRust\n");

    let cases: &[(usize, &[(usize, &str)])] = &[
        (0, &[]),
        (1, &[(1, "Rust Rust")]),
        (2, &[(1, "Rust Rust"), (3, "Rust")]),
    ];

    for flag in ["-m", "--max-count"] {
        for &(limit, expected) in cases {
            let output = command()
                .args(["-n", flag])
                .arg(limit.to_string())
                .arg("Rust")
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_selected_lines(output, &fixture.path, expected);
        }
    }
}

#[test]
fn per_file_limit_keeps_trailing_context_without_extending_it() {
    let fixture = Fixture::new(b"before\nRust\nRust\nRust\nRust\n");

    let output = command()
        .args(["-n", "-m", "1", "-B", "1", "-A", "2", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let path = fixture.path.display();
    let expected = format!(
        "{path}-1-before\n\
        {path}:2:Rust\n\
        {path}-3-Rust\n\
        {path}-4-Rust\n"
    );

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn per_file_limit_counts_inverted_selections() {
    let fixture = Fixture::new(b"Rust\nGo\nPython\nRust\n");

    let output = command()
        .args(["-n", "-v", "-m", "1", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(output, &fixture.path, &[(2, "Go")]);
}

#[test]
fn per_file_limit_does_not_read_past_required_trailing_context() {
    let fixture = Fixture::new(b"Rust\nafter\n\xff\n");

    for after in ["0", "1"] {
        let output = command()
            .args(["-n", "-m", "1", "-A", after, "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let path = fixture.path.display();
        let expected = if after == "0" {
            format!("{path}:1:Rust\n")
        } else {
            format!("{path}:1:Rust\n{path}-2-after\n")
        };

        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn per_file_limit_preserves_errors_encountered_before_stopping() {
    let cases: &[(&[u8], bool)] = &[(b"\xff\nRust\n", false), (b"Rust\n\xff\n", true)];

    for &(contents, printed_selection) in cases {
        let fixture = Fixture::new(contents);

        let output = command()
            .args(["-n", "-m", "1", "-A", "1", "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_operational_error(&output, &fixture.path);

        let expected = if printed_selection {
            format!("{}:1:Rust\n", fixture.path.display())
        } else {
            String::new()
        };

        assert_eq!(output.stdout, expected.as_bytes());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot read"), "{stderr}");
    }
}

#[test]
fn zero_global_limit_skips_discovery_but_still_validates_patterns() {
    let fixture = Fixture::new(b"");
    let missing = fixture.directory.join("missing-target.txt");

    let output = command()
        .args(["--max-results", "0", "Rust"])
        .arg(&missing)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let invalid = command()
        .args(["--max-results", "0", "["])
        .arg(&missing)
        .output()
        .expect("executable must start");

    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(stderr.contains("invalid search pattern"), "{stderr}");
    assert!(!stderr.contains("cannot discover files"), "{stderr}");
}

#[test]
fn global_limit_keeps_only_required_trailing_context() {
    let fixture = Fixture::new(b"before\nRust\nRust\nRust\n\xff\n");

    let output = command()
        .args(["-n", "--max-results", "1", "-B", "1", "-A", "2", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let path = fixture.path.display();
    let expected = format!(
        "{path}-1-before\n\
        {path}:2:Rust\n\
        {path}-3-Rust\n\
        {path}-4-Rust\n"
    );

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn global_limit_counts_inverted_selections() {
    let fixture = Fixture::new(b"Rust\nGo\nPython\nRust\n");

    let output = command()
        .args(["-n", "-v", "--max-results", "1", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(output, &fixture.path, &[(2, "Go")]);
}

#[test]
fn quiet_search_reports_selection_only_through_exit_status() {
    let fixture = Fixture::new(b"Rust\nGo\n");

    let cases: &[(&[&str], &str, i32)] = &[
        (&["-q"], "Rust", 0),
        (&["--quiet"], "Rust", 0),
        (&["-q"], "Python", 1),
        (&["-q", "-v"], "Rust", 0),
        (&["-q", "-v"], "", 1),
        (&["-q", "-m", "0"], "Rust", 1),
        (&["-q", "--max-results", "0"], "Rust", 1),
    ];

    for &(flags, pattern, expected_code) in cases {
        let output = command()
            .args(flags)
            .arg(pattern)
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "flags: {flags:?}, pattern: {pattern:?}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn quiet_search_stops_before_trailing_context_or_read_errors() {
    let fixture = Fixture::new(b"before\nRust\n\xff\n");

    for flag in ["-q", "--quiet"] {
        let output = command()
            .args([flag, "-n", "-C", "5", "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn quiet_search_preserves_errors_before_a_selection() {
    let fixture = Fixture::new(b"\xff\nRust\n");

    let output = command()
        .args(["-q", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_operational_error(&output, &fixture.path);
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot read"), "{stderr}");
}

#[test]
fn quiet_search_preserves_pattern_errors() {
    let target = Fixture::new(b"Rust\n");
    let invalid_file = Fixture::new(b"Rust\n\xff\n");

    let invalid_regex = command()
        .args(["-q", "--max-results", "0", "["])
        .arg(&target.path)
        .output()
        .expect("executable must start");

    assert_eq!(invalid_regex.status.code(), Some(2));
    assert!(invalid_regex.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&invalid_regex.stderr);
    assert!(stderr.contains("invalid search pattern"), "{stderr}");

    let invalid_patterns = command()
        .args(["-q", "-f"])
        .arg(&invalid_file.path)
        .arg(&target.path)
        .output()
        .expect("executable must start");

    assert_operational_error(&invalid_patterns, &invalid_file.path);
    assert!(invalid_patterns.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&invalid_patterns.stderr);
    assert!(stderr.contains("cannot read"), "{stderr}");
}

#[test]
fn file_list_modes_print_only_qualifying_paths() {
    let cases: &[(&[u8], bool)] = &[
        (b"Rust\nRust\n", true),
        (b"Go\nPython\n", false),
        (b"", false),
    ];

    for &(contents, has_selection) in cases {
        let fixture = Fixture::new(contents);

        for flag in ["-l", "--files-with-matches", "--files-without-match"] {
            let output = command()
                .args(["-n", "-C", "2", flag, "Rust"])
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            let qualifies = if flag == "--files-without-match" {
                !has_selection
            } else {
                has_selection
            };

            let expected_code = if qualifies { 0 } else { 1 };
            assert_eq!(
                output.status.code(),
                Some(expected_code),
                "flag: {flag}, stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());

            let expected = if qualifies {
                format!("{}\n", fixture.path.display())
            } else {
                String::new()
            };

            assert_eq!(output.stdout, expected.as_bytes());
        }
    }
}

#[test]
fn quiet_files_without_match_succeeds_only_for_qualifying_files() {
    let cases: &[(&[u8], i32)] = &[(b"Rust\n", 1), (b"Go\n", 0), (b"", 0)];

    for &(contents, expected_code) in cases {
        let fixture = Fixture::new(contents);

        let output = command()
            .args(["-q", "--files-without-match", "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn file_list_modes_use_inverted_line_selection() {
    let cases: &[(&[u8], bool)] = &[
        (b"Rust\nRust\n", false),
        (b"Rust\nGo\n", true),
        (b"", false),
    ];

    for &(contents, has_selection) in cases {
        let fixture = Fixture::new(contents);

        for flag in ["-l", "--files-without-match"] {
            let output = command()
                .args(["-v", flag, "Rust"])
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            let qualifies = if flag == "-l" {
                has_selection
            } else {
                !has_selection
            };

            assert_eq!(
                output.status.code(),
                Some(if qualifies { 0 } else { 1 }),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());

            let expected = if qualifies {
                format!("{}\n", fixture.path.display())
            } else {
                String::new()
            };

            assert_eq!(output.stdout, expected.as_bytes());
        }
    }
}

#[test]
fn file_list_modes_distinguish_read_errors_from_early_decisions() {
    let cases: &[(&[u8], i32, i32)] = &[
        (b"\xff\nRust\n", 2, 2),
        (b"Go\n\xff\n", 2, 2),
        (b"Rust\n\xff\n", 0, 1),
    ];

    for &(contents, with_code, without_code) in cases {
        let fixture = Fixture::new(contents);

        for (flag, expected_code) in [("-l", with_code), ("--files-without-match", without_code)] {
            for quiet in [false, true] {
                let mut process = command();
                process.arg(flag);

                if quiet {
                    process.arg("-q");
                }

                let output = process
                    .arg("Rust")
                    .arg(&fixture.path)
                    .output()
                    .expect("executable must start");

                assert_eq!(
                    output.status.code(),
                    Some(expected_code),
                    "flag: {flag}, quiet: {quiet}, stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );

                if expected_code == 2 {
                    assert_operational_error(&output, &fixture.path);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    assert!(stderr.contains("cannot read"), "{stderr}");
                } else {
                    assert!(output.stderr.is_empty());
                }

                let expected = if expected_code == 0 && !quiet {
                    format!("{}\n", fixture.path.display())
                } else {
                    String::new()
                };

                assert_eq!(output.stdout, expected.as_bytes());
            }
        }
    }
}

#[test]
fn count_modes_distinguish_selected_lines_from_match_spans() {
    let cases: &[(&[u8], u128, u128)] = &[
        (b"Rust Rust\nGo\nRust\n", 2, 3),
        (b"Go\nPython\n", 0, 0),
        (b"", 0, 0),
    ];

    for &(contents, line_count, match_count) in cases {
        let fixture = Fixture::new(contents);

        for (flag, expected_count) in [
            ("-c", line_count),
            ("--count", line_count),
            ("--count-matches", match_count),
        ] {
            let output = command()
                .args(["-n", "-C", "2", flag, "Rust"])
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            let expected_code = if line_count > 0 { 0 } else { 1 };

            assert_eq!(
                output.status.code(),
                Some(expected_code),
                "flag: {flag}, stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());

            let expected = format!("{}:{expected_count}\n", fixture.path.display());

            assert_eq!(output.stdout, expected.as_bytes());
        }
    }
}

#[test]
fn occurrence_counts_preserve_span_deduplication_and_zero_width_matches() {
    let cases: &[(&str, &[&str], u128)] = &[
        ("Rust Rust\nRust\n", &["Rust", "Rust"], 3),
        ("banana\n", &["ana", "nan", "ana"], 2),
        ("é\n", &[""], 2),
        ("é\n", &["", ""], 2),
    ];

    for &(contents, patterns, expected_count) in cases {
        let fixture = Fixture::new(contents.as_bytes());
        let mut process = command();
        process.arg("--count-matches");

        for pattern in patterns {
            process.arg("-e").arg(pattern);
        }

        let output = process
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "patterns: {patterns:?}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let expected = format!("{}:{expected_count}\n", fixture.path.display());

        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn selected_line_counts_support_inversion() {
    let fixture = Fixture::new(b"Rust\nGo\nPython\nRust\n");

    for flag in ["-c", "--count"] {
        let output = command()
            .args([flag, "-v", "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let expected = format!("{}:2\n", fixture.path.display());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn count_limits_apply_to_selected_lines_without_trailing_context() {
    let fixture = Fixture::new(b"Rust Rust\n\xff\n");

    for mode in ["-c", "--count-matches"] {
        for limit_flag in ["-m", "--max-results"] {
            for limit in ["0", "1"] {
                let output = command()
                    .args([mode, limit_flag, limit, "-A", "5", "Rust"])
                    .arg(&fixture.path)
                    .output()
                    .expect("executable must start");

                assert_eq!(
                    output.status.code(),
                    Some(if limit == "0" { 1 } else { 0 }),
                    "mode: {mode}, limit: {limit_flag} {limit}, stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(output.stderr.is_empty());

                let expected = if limit == "0" {
                    String::new()
                } else {
                    let count = if mode == "-c" { 1 } else { 2 };
                    format!("{}:{count}\n", fixture.path.display())
                };

                assert_eq!(output.stdout, expected.as_bytes());
            }
        }
    }
}

#[test]
fn quiet_count_modes_suppress_records_and_stop_at_a_selection() {
    let cases: &[(&[u8], i32)] = &[(b"Rust\n\xff\n", 0), (b"Go\n", 1), (b"", 1)];

    for &(contents, expected_code) in cases {
        let fixture = Fixture::new(contents);

        for mode in ["-c", "--count-matches"] {
            let output = command()
                .args(["-q", mode, "Rust"])
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_eq!(
                output.status.code(),
                Some(expected_code),
                "mode: {mode}, stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stdout.is_empty());
            assert!(output.stderr.is_empty());
        }
    }
}

#[test]
fn count_modes_report_read_errors_without_partial_counts() {
    for contents in [b"Rust\n\xff\n".as_slice(), b"Go\n\xff\n".as_slice()] {
        let fixture = Fixture::new(contents);

        for mode in ["-c", "--count-matches"] {
            let output = command()
                .args([mode, "Rust"])
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_operational_error(&output, &fixture.path);
            assert!(output.stdout.is_empty());

            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("cannot read"), "{stderr}");
        }
    }
}

#[test]
fn match_only_output_preserves_original_text_and_line_numbers() {
    let fixture = Fixture::new("é rUsT Rust\nGo\nRust\n".as_bytes());

    for flag in ["-o", "--only-matching"] {
        for numbered in [false, true] {
            let mut process = command();
            process.args([flag, "-i", "-C", "2"]);

            if numbered {
                process.arg("-n");
            }

            let output = process
                .arg("Rust")
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());

            let path = fixture.path.display();
            let expected: String = if numbered {
                format!(
                    "{path}:1:rUsT\n\
                    {path}:1:Rust\n\
                    {path}:3:Rust\n"
                )
            } else {
                format!(
                    "{path}:rUsT\n\
                    {path}:Rust\n\
                    {path}:Rust\n"
                )
            };

            assert_eq!(output.stdout, expected.as_bytes());
        }
    }
}

#[test]
fn zero_width_only_matches_select_lines_without_printing_records() {
    let fixture = Fixture::new("é\n".as_bytes());

    let output = command()
        .args(["-o", ""])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn match_only_handles_quiet_mode_and_no_selections() {
    let cases: &[(&[u8], bool, i32)] = &[
        (b"Rust\n\xff\n", true, 0),
        (b"Go\n", true, 1),
        (b"Go\n", false, 1),
        (b"", true, 1),
        (b"", false, 1),
    ];

    for &(contents, quiet, expected_code) in cases {
        let fixture = Fixture::new(contents);
        let mut process = command();
        process.arg("-o");

        if quiet {
            process.arg("-q");
        }

        let output = process
            .arg("Rust")
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn match_only_preserves_printed_spans_when_reading_fails() {
    let cases: &[(&[u8], bool)] = &[(b"Rust Rust\n\xff\n", true), (b"\xff\nRust\n", false)];

    for &(contents, printed_matches) in cases {
        let fixture = Fixture::new(contents);

        let output = command()
            .args(["-n", "-o", "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_operational_error(&output, &fixture.path);

        let path = fixture.path.display();
        let expected = if printed_matches {
            format!("{path}:1:Rust\n{path}:1:Rust\n")
        } else {
            String::new()
        };

        assert_eq!(output.stdout, expected.as_bytes());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot read"), "{stderr}");
    }
}

#[test]
fn filename_suppression_applies_to_selected_and_context_lines() {
    let fixture = Fixture::new(b"before\nRust\nafter\n");

    for (flag, expected) in [
        ("-h", "before\nRust\nafter\n"),
        ("--no-filename", "before\nRust\nafter\n"),
        ("-hn", "1-before\n2:Rust\n3-after\n"),
    ] {
        let output = command()
            .args([flag, "-C", "1", "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn filename_suppression_applies_to_count_and_match_only_output() {
    let fixture = Fixture::new(b"Rust Rust\nGo\nRust\n");

    let cases: &[(&[&str], &str)] = &[
        (&["-hc"], "2\n"),
        (&["--no-filename", "--count"], "2\n"),
        (&["-h", "--count-matches"], "3\n"),
        (&["-hnc"], "2\n"),
        (&["-ho"], "Rust\nRust\nRust\n"),
        (&["--no-filename", "--only-matching"], "Rust\nRust\nRust\n"),
        (&["-hno"], "1:Rust\n1:Rust\n3:Rust\n"),
    ];

    for &(flags, expected) in cases {
        let output = command()
            .args(flags)
            .arg("Rust")
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "flags: {flags:?}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes(), "flags: {flags:?}");
    }
}

#[test]
fn explicit_filename_inclusion_preserves_prefixes_across_modes() {
    let fixture = Fixture::new(b"Rust Rust\n");
    let path = fixture.path.display();

    let cases: &[(&[&str], String)] = &[
        (&["-n"], format!("{path}:1:Rust Rust\n")),
        (&["-no"], format!("{path}:1:Rust\n{path}:1:Rust\n")),
        (&["-c"], format!("{path}:1\n")),
        (&["--count-matches"], format!("{path}:2\n")),
    ];

    for filename_flag in ["-H", "--with-filename"] {
        for (flags, expected) in cases {
            let output = command()
                .arg(filename_flag)
                .args(*flags)
                .arg("Rust")
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, expected.as_bytes());
        }
    }
}

#[test]
fn file_list_modes_always_print_paths_despite_filename_controls() {
    for (mode, contents) in [
        ("-l", b"Rust\n".as_slice()),
        ("--files-without-match", b"Go\n".as_slice()),
    ] {
        let fixture = Fixture::new(contents);

        for filename_flag in ["-H", "--with-filename", "-h", "--no-filename"] {
            let output = command()
                .args([mode, filename_flag, "Rust"])
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());

            let expected = format!("{}\n", fixture.path.display());
            assert_eq!(output.stdout, expected.as_bytes());
        }
    }
}

#[test]
fn filename_suppression_preserves_diagnostic_paths() {
    let fixture = Fixture::new(b"\xff\n");

    for quiet in [false, true] {
        let mut process = command();
        process.arg("-h");

        if quiet {
            process.arg("-q");
        }

        let output = process
            .arg("Rust")
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_operational_error(&output, &fixture.path);
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot read"), "{stderr}");
    }
}

#[test]
fn column_output_uses_original_byte_offsets_for_lines_and_spans() {
    let fixture = Fixture::new("é Rust Rust\n".as_bytes());
    let path = fixture.path.display();

    let cases: &[(&[&str], String)] = &[
        (&[], format!("{path}:1:4:é Rust Rust\n")),
        (&["-h"], String::from("1:4:é Rust Rust\n")),
        (&["-o"], format!("{path}:1:4:Rust\n{path}:1:9:Rust\n")),
        (&["-ho"], String::from("1:4:Rust\n1:9:Rust\n")),
    ];

    for (flags, expected) in cases {
        let output = command()
            .arg("--column")
            .args(*flags)
            .arg("Rust")
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn context_columns_use_one_and_selected_lines_use_match_starts() {
    let fixture = Fixture::new("before\né Rust\nafter\n".as_bytes());

    let output = command()
        .args(["--column", "-h", "-C", "1", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        "1-1-before\n2:4:é Rust\n3-1-after\n".as_bytes()
    );
}

#[test]
fn column_output_handles_zero_width_and_inverted_selections() {
    let fixture = Fixture::new("é Go\n".as_bytes());

    let cases: &[(&[&str], &str, &str)] = &[
        (&[], "", "1:1:é Go\n"),
        (&[], "$", "1:6:é Go\n"),
        (&["-v"], "Rust", "1:1:é Go\n"),
        (&["-o"], "$", ""),
    ];

    for &(flags, pattern, expected) in cases {
        let output = command()
            .args(["--column", "-h"])
            .args(flags)
            .arg(pattern)
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn count_file_list_and_quiet_modes_ignore_column_formatting() {
    let fixture = Fixture::new(b"Rust Rust\n");
    let path = fixture.path.display();

    let cases: &[(&[&str], &str, String)] = &[
        (&["-c"], "Rust", format!("{path}:1\n")),
        (&["--count-matches"], "Rust", format!("{path}:2\n")),
        (&["-hc"], "Rust", String::from("1\n")),
        (&["-l"], "Rust", format!("{path}\n")),
        (&["--files-without-match"], "Go", format!("{path}\n")),
        (&["-q"], "Rust", String::new()),
    ];

    for (flags, pattern, expected) in cases {
        let output = command()
            .arg("--column")
            .args(*flags)
            .arg(pattern)
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn headings_replace_repeated_filename_prefixes() {
    let fixture = Fixture::new("é Rust Rust\n".as_bytes());
    let path = fixture.path.display();

    let cases: &[(&[&str], String)] = &[
        (&[], format!("{path}\né Rust Rust\n")),
        (&["-Hn"], format!("{path}\n1:é Rust Rust\n")),
        (&["--column"], format!("{path}\n1:4:é Rust Rust\n")),
        (&["-o"], format!("{path}\nRust\nRust\n")),
        (&["-o", "--column"], format!("{path}\n1:4:Rust\n1:9:Rust\n")),
    ];

    for (flags, expected) in cases {
        let output = command()
            .arg("--heading")
            .args(*flags)
            .arg("Rust")
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn heading_context_groups_keep_one_heading_and_internal_separators() {
    let fixture = Fixture::new(b"before\nRust\nafter\nomitted\nbefore again\nRust\nafter again\n");

    let output = command()
        .args(["--heading", "-n", "-C", "1", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let expected = format!(
        "{}\n\
        1-before\n\
        2:Rust\n\
        3-after\n\
        --\n\
        5-before again\n\
        6:Rust\n\
        7-after again\n",
        fixture.path.display()
    );

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn headings_are_omitted_when_no_visible_line_records_are_emitted() {
    let fixture = Fixture::new(b"Rust Rust\n");
    let path = fixture.path.display();

    let cases: &[(&[&str], &str, i32, String)] = &[
        (&[], "Go", 1, String::new()),
        (&["-o"], "$", 0, String::new()),
        (&["-o"], "Go", 1, String::new()),
        (&["-q"], "Rust", 0, String::new()),
        (&["-c"], "Rust", 0, format!("{path}:1\n")),
        (&["--count-matches"], "Rust", 0, format!("{path}:2\n")),
        (&["-l"], "Rust", 0, format!("{path}\n")),
        (&["--files-without-match"], "Go", 0, format!("{path}\n")),
    ];

    for (flags, pattern, expected_code, expected) in cases {
        let output = command()
            .arg("--heading")
            .args(*flags)
            .arg(pattern)
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(*expected_code),
            "flags: {flags:?}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn read_errors_preserve_visible_headings_without_creating_empty_headings() {
    let cases: &[(&[u8], bool)] = &[(b"Rust\n\xff\n", true), (b"\xff\nRust\n", false)];

    for &(contents, printed_result) in cases {
        let fixture = Fixture::new(contents);

        for flag in ["-n", "-no"] {
            let output = command()
                .args(["--heading", flag, "Rust"])
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_operational_error(&output, &fixture.path);

            let expected = if printed_result {
                format!("{}\n1:Rust\n", fixture.path.display())
            } else {
                String::new()
            };

            assert_eq!(output.stdout, expected.as_bytes());

            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("cannot read"), "{stderr}");
        }
    }
}

#[test]
fn headings_respect_zero_limits_and_match_only_stopping() {
    let fixture = Fixture::new(b"Rust Rust\n\xff\n");

    for limit_flag in ["-m", "--max-results"] {
        for limit in ["0", "1"] {
            for only_matching in [false, true] {
                let mut process = command();
                process.args(["--heading", "-n", limit_flag, limit]);

                if only_matching {
                    process.args(["-o", "-A", "5"]);
                }

                let output = process
                    .arg("Rust")
                    .arg(&fixture.path)
                    .output()
                    .expect("executable must start");

                assert_eq!(
                    output.status.code(),
                    Some(if limit == "0" { 1 } else { 0 }),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(output.stderr.is_empty());

                let expected = if limit == "0" {
                    String::new()
                } else if only_matching {
                    format!("{}\n1:Rust\n1:Rust\n", fixture.path.display())
                } else {
                    format!("{}\n1:Rust Rust\n", fixture.path.display())
                };

                assert_eq!(output.stdout, expected.as_bytes());
            }
        }
    }
}

#[test]
fn null_output_delimits_filenames_across_output_modes() {
    let fixture = Fixture::new(b"Rust Rust\n");
    let path = fixture.path.display();

    let cases: &[(&[&str], &str, String)] = &[
        (&[], "Rust", format!("{path}\0Rust Rust\n")),
        (&["-n"], "Rust", format!("{path}\01:Rust Rust\n")),
        (&["--column"], "Rust", format!("{path}\01:1:Rust Rust\n")),
        (&["-o"], "Rust", format!("{path}\0Rust\n{path}\0Rust\n")),
        (&["-c"], "Rust", format!("{path}\01\n")),
        (&["--count-matches"], "Rust", format!("{path}\02\n")),
        (&["-l"], "Rust", format!("{path}\0")),
        (&["--files-without-match"], "Go", format!("{path}\0")),
    ];

    for null_flag in ["-0", "--null"] {
        for (flags, pattern, expected) in cases {
            let output = command()
                .arg(null_flag)
                .args(*flags)
                .arg(pattern)
                .arg(&fixture.path)
                .output()
                .expect("executable must start");

            assert_eq!(
                output.status.code(),
                Some(0),
                "flags: {flags:?}, stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, expected.as_bytes());
        }
    }
}

#[test]
fn null_context_output_preserves_numeric_and_group_separators() {
    let fixture = Fixture::new(b"before\nRust\nafter\nomitted\nbefore again\nRust\nafter again\n");

    let output = command()
        .args(["-0", "--column", "-C", "1", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let path = fixture.path.display();
    let expected = format!(
        "{path}\01-1-before\n\
         {path}\02:1:Rust\n\
         {path}\03-1-after\n\
         --\n\
         {path}\05-1-before again\n\
         {path}\06:1:Rust\n\
         {path}\07-1-after again\n"
    );

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn null_output_respects_filename_suppression_and_quiet_mode() {
    let fixture = Fixture::new(b"Rust Rust\n");
    let path = fixture.path.display();

    let cases: &[(&[&str], String)] = &[
        (&["-h"], String::from("Rust Rust\n")),
        (&["-hn"], String::from("1:Rust Rust\n")),
        (&["-ho"], String::from("Rust\nRust\n")),
        (&["-hc"], String::from("1\n")),
        (&["-h", "--count-matches"], String::from("2\n")),
        (&["-hl"], format!("{path}\0")),
        (&["-q"], String::new()),
    ];

    for (flags, expected) in cases {
        let output = command()
            .arg("--null")
            .args(*flags)
            .arg("Rust")
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn null_output_preserves_partial_results_and_readable_diagnostics() {
    let fixture = Fixture::new(b"Rust\n\xff\n");

    let output = command()
        .args(["-0", "-n", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_operational_error(&output, &fixture.path);

    let expected = format!("{}\01:Rust\n", fixture.path.display());
    assert_eq!(output.stdout, expected.as_bytes());
    assert!(!output.stderr.contains(&0));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot read"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn null_output_preserves_non_utf8_paths_with_embedded_newlines() {
    use std::os::unix::ffi::OsStrExt;

    let mut fixture = Fixture::new(b"Rust\n");
    let renamed = fixture
        .directory
        .join(std::ffi::OsStr::from_bytes(b"sample-\xff\n.txt"));

    if !rename_non_utf8_fixture(&mut fixture, renamed) {
        return;
    }

    for file_list in [false, true] {
        let mut process = command();
        process.args(["-0", "-n"]);

        if file_list {
            process.arg("-l");
        }

        let output = process
            .arg("Rust")
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let mut expected = fixture.path.as_os_str().as_bytes().to_vec();
        expected.push(0);

        if !file_list {
            expected.extend_from_slice(b"1:Rust\n");
        }

        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn a_relative_dash_filename_is_not_treated_as_stdin() {
    let mut fixture = Fixture::new(b"Rust\n");
    let renamed = fixture.directory.join("-");

    fs::rename(&fixture.path, &renamed).expect("fixture must be renamed");
    fixture.path = renamed;

    let target = Path::new(".").join("-");

    let output = command()
        .current_dir(&fixture.directory)
        .arg("Rust")
        .arg(&target)
        .output()
        .expect("executable must start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let expected = format!("{}:Rust\n", target.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn combines_argument_file_and_stdin_pattern_sources() {
    use std::io::Write;
    use std::process::Stdio;

    let target = Fixture::new(b"Rust makes ownership explicit.\nGo\nPython\n");
    let pattern_file = Path::new("tests")
        .join("fixtures")
        .join("text")
        .join("demo.txt");

    let mut child = command()
        .args(["-n", "-e", "^Python$", "-f"])
        .arg(&pattern_file)
        .args(["-f", "-"])
        .arg(&target.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executable must start");

    {
        let mut input = child.stdin.take().expect("child stdin must be available");
        input
            .write_all(b"^Go$\n")
            .expect("patterns must be written");
    }

    let output = child
        .wait_with_output()
        .expect("child output must be collected");

    assert_selected_lines(
        output,
        &target.path,
        &[
            (1, "Rust makes ownership explicit."),
            (2, "Go"),
            (3, "Python"),
        ],
    );
}

#[test]
fn a_relative_dash_pattern_file_is_not_treated_as_stdin() {
    let mut patterns = Fixture::new(b"Rust\n");
    let renamed = patterns.directory.join("-");

    fs::rename(&patterns.path, &renamed).expect("pattern fixture must be renamed");
    patterns.path = renamed;

    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("text")
        .join("demo.txt");

    let pattern_path = Path::new(".").join("-");

    let output = command()
        .current_dir(&patterns.directory)
        .args(["-n", "-f"])
        .arg(&pattern_path)
        .arg(&target)
        .output()
        .expect("executable must start");

    assert_selected_lines(
        output,
        &target,
        &[
            (2, "Rust makes ownership explicit."),
            (4, "Rust rewards careful design."),
        ],
    );
}

#[test]
fn file_enumeration_reports_a_missing_target() {
    let fixture = Fixture::new(b"");
    let missing = fixture.directory.join("missing-directory");

    let output = command()
        .arg("--files")
        .arg(&missing)
        .output()
        .expect("executable must start");

    assert_operational_error(&output, &missing);
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot discover files"), "{stderr}");
}

#[test]
fn path_list_source_errors_precede_all_target_output() {
    let target = Fixture::new(b"Rust\n");
    let contents = format!("{}\n", target.path.display());
    let valid = Fixture::new(contents.as_bytes());
    let invalid = Fixture::new(b"bad\0name\n");
    let missing = invalid.directory.join("missing.paths");

    let modes: &[&[&str]] = &[&["Rust"], &["Rust", "--max-results", "0"], &["--files"]];

    for (bad_source, diagnostic) in [
        (invalid.path.as_path(), "cannot read"),
        (missing.as_path(), "cannot open"),
    ] {
        for flags in modes {
            let output = command()
                .args(*flags)
                .arg("--files-from")
                .arg(&valid.path)
                .arg("--files-from")
                .arg(bad_source)
                .output()
                .expect("executable must start");

            assert_operational_error(&output, bad_source);
            assert!(output.stdout.is_empty());

            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains(diagnostic), "{stderr}");
        }
    }
}

#[cfg(unix)]
#[test]
fn listed_unix_paths_preserve_non_utf8_bytes_through_null_output() {
    use std::os::unix::ffi::OsStrExt;

    let mut target = Fixture::new(b"Rust\n");
    let renamed = target
        .directory
        .join(std::ffi::OsStr::from_bytes(b"sample-\xff #.txt"));

    if !rename_non_utf8_fixture(&mut target, renamed) {
        return;
    }

    let mut contents = target.path.as_os_str().as_bytes().to_vec();
    contents.push(b'\n');
    let list = Fixture::new(&contents);

    for enumerate in [false, true] {
        let mut process = command();
        process.arg("-0");

        if enumerate {
            process.arg("--files");
        } else {
            process.args(["-n", "Rust"]);
        }

        let output = process
            .arg("--files-from")
            .arg(&list.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let mut expected = target.path.as_os_str().as_bytes().to_vec();
        expected.push(0);

        if !enumerate {
            expected.extend_from_slice(b"1:Rust\n");
        }

        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn redirected_color_defaults_to_plain_and_can_be_forced() {
    let fixture = Fixture::new("é Rust!\n".as_bytes());

    let plain_cases: &[&[&str]] = &[&[], &["--color=auto"], &["--color=never"]];

    for flags in plain_cases {
        let output = command()
            .args(*flags)
            .args(["-n", "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_selected_lines(output, &fixture.path, &[(1, "é Rust!")]);
    }

    let output = command()
        .args(["--color=always", "-n", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(output, &fixture.path, &[(1, "é \x1b[1;31mRust\x1b[0m!")]);
}

#[test]
fn forced_color_keeps_context_and_numeric_prefixes_plain() {
    let fixture = Fixture::new("before\né Rust Rust\nafter\n".as_bytes());

    let output = command()
        .args(["--color=always", "-h", "--column", "-C", "1", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.stderr.is_empty());

    let expected = concat!(
        "1-1-before\n",
        "2:4:é \x1b[1;31mRust\x1b[0m \x1b[1;31mRust\x1b[0m\n",
        "3-1-after\n",
    );

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn forced_match_only_color_preserves_original_byte_columns() {
    let fixture = Fixture::new("é Rust Rust\n".as_bytes());

    let output = command()
        .args(["--color=always", "-h", "--column", "-o", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.stderr.is_empty());

    let expected = concat!("1:4:\x1b[1;31mRust\x1b[0m\n", "1:9:\x1b[1;31mRust\x1b[0m\n",);

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn forced_color_merges_full_line_overlaps_but_keeps_match_only_records() {
    let fixture = Fixture::new(b"abcd\n");

    let output = command()
        .args(["--color=always", "-n", "-e", "abc", "-e", "bcd"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(output, &fixture.path, &[(1, "\x1b[1;31mabcd\x1b[0m")]);

    let output = command()
        .args(["--color=always", "-no", "-e", "abc", "-e", "bcd"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(
        output,
        &fixture.path,
        &[(1, "\x1b[1;31mabc\x1b[0m"), (1, "\x1b[1;31mbcd\x1b[0m")],
    );
}

#[test]
fn forced_color_does_not_render_zero_width_matches() {
    let fixture = Fixture::new(b"Rust\nGo\n");

    let output = command()
        .args(["--color=always", "-n", "^"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(output, &fixture.path, &[(1, "Rust"), (2, "Go")]);

    let output = command()
        .args(["--color=always", "-o", "^"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn forced_color_leaves_non_match_output_plain() {
    let fixture = Fixture::new(b"Rust Rust\nGo\n");
    let path = fixture.path.display();

    let cases = [
        (vec!["-c", "Rust"], format!("{path}:1\n")),
        (vec!["--count-matches", "Rust"], format!("{path}:2\n")),
        (vec!["-l", "Rust"], format!("{path}\n")),
        (vec!["--files-without-match", "absent"], format!("{path}\n")),
        (vec!["--files"], format!("{path}\n")),
        (vec!["-q", "Rust"], String::new()),
        (vec!["-nv", "Rust"], format!("{path}:2:Go\n")),
    ];

    for (arguments, expected) in cases {
        let output = command()
            .arg("--color=always")
            .args(&arguments)
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "arguments: {arguments:?}; stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            expected.as_bytes(),
            "arguments: {arguments:?}",
        );
    }
}

#[test]
fn binary_detection_precedes_utf8_decoding() {
    let fixture = Fixture::new(b"\xff\0Rust\n");

    let output = command()
        .arg("Rust")
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let output = command()
        .args(["--text", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_operational_error(&output, &fixture.path);
    assert!(output.stdout.is_empty());
}

#[test]
fn text_flag_preserves_nul_in_matching_utf8_content() {
    let fixture = Fixture::new(b"Rust\0ok\n");

    let output = command()
        .args(["--text", "-n", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_selected_lines(output, &fixture.path, &[(1, "Rust\0ok")]);
}

#[test]
fn late_binary_detection_preserves_printed_matches_and_context() {
    let fixture = Fixture::new(b"before\nRust\ncontext\nRu\0st\nRust\n");

    let output = command()
        .args(["-n", "-C", "1", "Rust"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let path = fixture.path.display();
    let expected = format!(
        "{path}-1-before\n\
         {path}:2:Rust\n\
         {path}-3-context\n"
    );

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn binary_stop_suppresses_incomplete_counts() {
    let fixture = Fixture::new(b"Rust Rust\nGo\0\nRust\n");

    for (mode, text_count) in [("-c", 2), ("--count-matches", 3)] {
        let output = command()
            .args([mode, "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "mode: {mode}; stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());

        let output = command()
            .args(["--text", mode, "Rust"])
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());

        let expected = format!("{}:{text_count}\n", fixture.path.display());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn binary_stop_does_not_qualify_a_file_without_matches() {
    let fixture = Fixture::new(b"Go\n\0Rust\n");

    let output = command()
        .args(["--files-without-match", "absent"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let output = command()
        .args(["--text", "--files-without-match", "absent"])
        .arg(&fixture.path)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}\n", fixture.path.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn early_exits_leave_later_binary_and_invalid_bytes_unchecked() {
    let fixture = Fixture::new(b"Rust\n\xff\0\n");
    let path = fixture.path.display();

    let cases = [
        (vec!["-q"], 0, String::new()),
        (vec!["-l"], 0, format!("{path}\n")),
        (vec!["-n", "-m", "1"], 0, format!("{path}:1:Rust\n")),
        (
            vec!["-n", "--max-results", "1"],
            0,
            format!("{path}:1:Rust\n"),
        ),
        (vec!["-c", "-m", "1"], 0, format!("{path}:1\n")),
        (vec!["--files-without-match"], 1, String::new()),
        (vec!["--max-results", "0"], 1, String::new()),
    ];

    for (arguments, expected_code, expected_output) in cases {
        let output = command()
            .arg("--text")
            .args(&arguments)
            .arg("Rust")
            .arg(&fixture.path)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "arguments: {arguments:?}; stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            expected_output.as_bytes(),
            "arguments: {arguments:?}",
        );
    }
}
