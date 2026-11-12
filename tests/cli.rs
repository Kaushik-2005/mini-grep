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
    let cases: &[&[&str]] = &[&[], &["Rust"], &["Rust", "file.txt", "extra.txt"]];

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

    fs::rename(&fixture.path, &renamed).expect("fixture must be renamed");
    fixture.path = renamed;

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
        "<PATH>",
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
fn rejects_unknown_reserved_and_repeated_flags() {
    let fixture = Fixture::new(b"Rust\n");

    for flag in ["--unknown", "-h", "--line-numbers", "-ii"] {
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
    assert!(stderr.contains("<PATH>"), "{stderr}");
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

    let missing_path = command()
        .args(["-e", "Rust"])
        .output()
        .expect("executable must start");

    let stdin_patterns = command()
        .args(["-f", "-"])
        .arg(&target.path)
        .output()
        .expect("executable must start");

    let stderr = String::from_utf8_lossy(&stdin_patterns.stderr);
    assert!(stderr.contains("stdin"), "{stderr}");

    for output in [extra_pattern, missing_path, stdin_patterns] {
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("error:"), "{stderr}");
        assert!(stderr.contains("Usage:"), "{stderr}");
    }
}
