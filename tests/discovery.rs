use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after the Unix epoch")
            .as_nanos();

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "mini-grep-tree-{}-{timestamp}-{id}",
            std::process::id()
        ));

        fs::create_dir(&root).expect("fixture directory must be created");
        Self { root }
    }

    fn write(&self, relative: &str, contents: &[u8]) -> PathBuf {
        let path = relative
            .split('/')
            .fold(self.root.clone(), |path, component| path.join(component));

        fs::create_dir_all(path.parent().expect("fixture file must have a parent"))
            .expect("fixture directories must be created");

        fs::write(&path, contents).expect("fixture file must be written");
        path
    }

    fn search(&self) -> Output {
        self.search_with(&[])
    }

    fn search_with(&self, flags: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_mini-grep"))
            .args(flags)
            .args(["-n", "Rust"])
            .arg(&self.root)
            .output()
            .expect("executable must start")
    }

    fn list_files(&self, flags: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_mini-grep"))
            .arg("--files")
            .args(flags)
            .arg(&self.root)
            .output()
            .expect("executable must start")
    }
}

fn assert_selected_files(output: Output, paths: &[PathBuf]) {
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");

    let mut actual: Vec<_> = stdout.lines().map(str::to_owned).collect();
    let mut expected: Vec<_> = paths
        .iter()
        .map(|path| format!("{}:1:Rust", path.display()))
        .collect();

    actual.sort();
    expected.sort();

    assert_eq!(actual, expected);
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn no_ignore_disables_all_rules_and_takes_precedence_in_either_order() {
    let fixture = Fixture::new();
    fixture.write(".ignore", b"plain.txt\n");
    fixture.write(".gitignore", b"git.txt\n");

    let plain = fixture.write("plain.txt", b"Rust\n");
    let git = fixture.write("git.txt", b"Rust\n");
    let visible = fixture.write("visible.txt", b"Rust\n");
    fixture.write(".hidden.txt", b"Rust\n");

    let cases: &[&[&str]] = &[
        &["--no-ignore"],
        &["--no-ignore", "--no-ignore-vcs"],
        &["--no-ignore-vcs", "--no-ignore"],
    ];

    for flags in cases {
        assert_selected_files(
            fixture.search_with(flags),
            &[plain.clone(), git.clone(), visible.clone()],
        );
    }
}

#[test]
fn no_ignore_vcs_retains_plain_ignore_rules() {
    let fixture = Fixture::new();
    fixture.write(".ignore", b"plain.txt\n");
    fixture.write(".gitignore", b"git.txt\n");
    fixture.write(".git/info/exclude", b"excluded.txt\n");

    fixture.write("plain.txt", b"Rust\n");
    let git = fixture.write("git.txt", b"Rust\n");
    let excluded = fixture.write("excluded.txt", b"Rust\n");
    let visible = fixture.write("visible.txt", b"Rust\n");

    assert_selected_files(
        fixture.search_with(&["--no-ignore-vcs"]),
        &[git, excluded, visible],
    );
}

#[test]
fn disabled_ignore_sources_are_not_parsed_or_audited() {
    for relative in [".ignore", ".gitignore", ".git/info/exclude"] {
        let fixture = Fixture::new();
        fixture.write(relative, b"[z-a]\n\xff\n");
        let visible = fixture.write("visible.txt", b"Rust\n");

        assert_selected_files(
            fixture.search_with(&["--no-ignore"]),
            std::slice::from_ref(&visible),
        );

        if relative != ".ignore" {
            assert_selected_files(
                fixture.search_with(&["--no-ignore-vcs"]),
                std::slice::from_ref(&visible),
            );
        }
    }
}

#[test]
fn hidden_and_ignore_switches_are_independent() {
    let fixture = Fixture::new();
    fixture.write(".ignore", b".hidden.txt\n");

    let hidden = fixture.write(".hidden.txt", b"Rust\n");
    let visible = fixture.write("visible.txt", b"Rust\n");

    assert_selected_files(
        fixture.search_with(&["--no-ignore"]),
        std::slice::from_ref(&visible),
    );

    assert_selected_files(
        fixture.search_with(&["--no-ignore", "--hidden"]),
        &[hidden, visible],
    );
}

#[test]
fn searches_nested_files_with_independent_line_numbers() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"Rust\n");
    let second = fixture.write("nested/second.txt", b"other\nRust\n");
    fixture.write("nested/no-match.txt", b"Go\n");

    let output = fixture.search();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");

    let mut actual: Vec<_> = stdout.lines().map(str::to_owned).collect();
    let mut expected = vec![
        format!("{}:1:Rust", first.display()),
        format!("{}:2:Rust", second.display()),
    ];

    actual.sort();
    expected.sort();

    assert_eq!(actual, expected);
}

#[test]
fn empty_directory_returns_no_selection() {
    let fixture = Fixture::new();
    let output = fixture.search();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn file_errors_preserve_successful_results_and_return_two() {
    let fixture = Fixture::new();
    let readable = fixture.write("readable.txt", b"Rust\n");
    let invalid = fixture.write("nested/invalid.txt", b"\xff\n");

    let output = fixture.search();

    assert_eq!(output.status.code(), Some(2));

    let expected = format!("{}:1:Rust\n", readable.display());
    assert_eq!(output.stdout, expected.as_bytes());

    let stderr = String::from_utf8_lossy(&output.stderr);
    let invalid_path = invalid.display().to_string();

    assert!(stderr.contains("cannot read"), "{stderr}");
    assert!(stderr.contains(invalid_path.as_str()), "{stderr}");
    assert!(!stderr.contains("panicked at"), "{stderr}");
}

#[test]
fn hidden_entries_are_skipped_by_default() {
    let fixture = Fixture::new();
    let visible = fixture.write("visible.txt", b"Rust\n");
    fixture.write(".hidden.txt", b"Rust\n");
    fixture.write(".private/secret.txt", b"Rust\n");

    let output = fixture.search();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:1:Rust\n", visible.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn hidden_flag_includes_hidden_files_and_directories() {
    let fixture = Fixture::new();
    let visible = fixture.write("visible.txt", b"Rust\n");
    let hidden = fixture.write(".hidden.txt", b"Rust\n");
    let nested = fixture.write(".private/secret.txt", b"Rust\n");

    let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .args(["--hidden", "-n", "Rust"])
        .arg(&fixture.root)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");

    let mut actual: Vec<_> = stdout.lines().map(str::to_owned).collect();
    let mut expected = vec![
        format!("{}:1:Rust", visible.display()),
        format!("{}:1:Rust", hidden.display()),
        format!("{}:1:Rust", nested.display()),
    ];

    actual.sort();
    expected.sort();

    assert_eq!(actual, expected);
}

#[test]
fn explicit_hidden_file_is_searched_without_hidden_flag() {
    let fixture = Fixture::new();
    let hidden = fixture.write(".hidden.txt", b"Rust\n");

    let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .args(["-n", "Rust"])
        .arg(&hidden)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:1:Rust\n", hidden.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn local_ignore_files_exclude_matching_paths_without_a_git_repository() {
    let fixture = Fixture::new();
    fixture.write(".ignore", b"plain-ignored.txt\n");
    fixture.write(".gitignore", b"git-ignored.txt\n");
    fixture.write("plain-ignored.txt", b"Rust\n");
    fixture.write("git-ignored.txt", b"Rust\n");
    let visible = fixture.write("visible.txt", b"Rust\n");

    let output = fixture.search();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:1:Rust\n", visible.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn nested_gitignore_rules_and_negation_are_respected() {
    let fixture = Fixture::new();
    fixture.write(".gitignore", b"*.log\n!important.log\n");
    fixture.write("ignored.log", b"Rust\n");
    let important = fixture.write("important.log", b"Rust\n");

    fixture.write("nested/.gitignore", b"!keep.log\n");
    let keep = fixture.write("nested/keep.log", b"Rust\n");
    fixture.write("nested/drop.log", b"Rust\n");

    let output = fixture.search();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");

    let mut actual: Vec<_> = stdout.lines().map(str::to_owned).collect();
    let mut expected = vec![
        format!("{}:1:Rust", important.display()),
        format!("{}:1:Rust", keep.display()),
    ];

    actual.sort();
    expected.sort();

    assert_eq!(actual, expected);
}

#[test]
fn plain_ignore_rules_override_gitignore_whitelisting() {
    let fixture = Fixture::new();
    fixture.write(".ignore", b"blocked.txt\n");
    fixture.write(".gitignore", b"!blocked.txt\n");
    fixture.write("blocked.txt", b"Rust\n");

    let output = fixture.search();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn repository_exclude_rules_are_respected() {
    let fixture = Fixture::new();
    fixture.write(".git/info/exclude", b"blocked.txt\n");
    fixture.write("blocked.txt", b"Rust\n");
    let visible = fixture.write("visible.txt", b"Rust\n");

    let output = fixture.search();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:1:Rust\n", visible.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn explicitly_supplied_ignored_file_is_searched() {
    let fixture = Fixture::new();
    fixture.write(".gitignore", b"ignored.txt\n");
    let ignored = fixture.write("ignored.txt", b"Rust\n");

    let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .args(["-n", "Rust"])
        .arg(&ignored)
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let expected = format!("{}:1:Rust\n", ignored.display());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn ignore_syntax_errors_are_reported_without_losing_matches() {
    let fixture = Fixture::new();
    let ignore_file = fixture.write(".ignore", b"[z-a]\n");
    let visible = fixture.write("visible.txt", b"Rust\n");

    let output = fixture.search();

    assert_eq!(output.status.code(), Some(2));

    let expected = format!("{}:1:Rust\n", visible.display());
    assert_eq!(output.stdout, expected.as_bytes());

    let stderr = String::from_utf8_lossy(&output.stderr);
    let ignore_path = ignore_file.display().to_string();

    assert!(stderr.contains("cannot discover files"), "{stderr}");
    assert!(stderr.contains(ignore_path.as_str()), "{stderr}");
}

#[test]
fn ignore_file_read_errors_are_reported_without_losing_matches() {
    for relative in [".ignore", ".gitignore", ".git/info/exclude"] {
        let fixture = Fixture::new();
        let ignore_file = fixture.write(relative, b"blocked.txt\n\xff\n");
        fixture.write("blocked.txt", b"Rust\n");
        let visible = fixture.write("visible.txt", b"Rust\n");

        let output = fixture.search();

        assert_eq!(output.status.code(), Some(2), "file: {relative}");

        let expected = format!("{}:1:Rust\n", visible.display());
        assert_eq!(output.stdout, expected.as_bytes());

        let stderr = String::from_utf8_lossy(&output.stderr);
        let ignore_path = ignore_file.display().to_string();

        assert!(stderr.contains("cannot discover files"), "{stderr}");
        assert!(stderr.contains(ignore_path.as_str()), "{stderr}");
    }
}

#[cfg(windows)]
#[test]
fn locked_ignore_file_is_reported_without_losing_matches() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    let fixture = Fixture::new();
    let ignore_file = fixture.write(".ignore", b"blocked.txt\n");
    let visible = fixture.write("visible.txt", b"Rust\n");

    let lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&ignore_file)
        .expect("exclusive ignore-file handle must open");

    let output = fixture.search();

    drop(lock);

    assert_eq!(output.status.code(), Some(2));

    let expected = format!("{}:1:Rust\n", visible.display());
    assert_eq!(output.stdout, expected.as_bytes());

    let stderr = String::from_utf8_lossy(&output.stderr);
    let ignore_path = ignore_file.display().to_string();

    assert!(stderr.contains("cannot discover files"), "{stderr}");
    assert!(stderr.contains(ignore_path.as_str()), "{stderr}");
}

#[test]
fn maximum_depth_counts_the_root_as_zero() {
    let fixture = Fixture::new();
    let direct = fixture.write("direct.txt", b"Rust\n");
    let nested = fixture.write("nested/second.txt", b"Rust\n");
    fixture.write("nested/deeper/third.txt", b"Rust\n");

    let cases = [
        ("0", vec![]),
        ("1", vec![direct.clone()]),
        ("2", vec![direct, nested]),
    ];

    for (depth, expected) in cases {
        let output = fixture.search_with(&["--max-depth", depth]);

        if expected.is_empty() {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(output.stderr.is_empty());
        } else {
            assert_selected_files(output, &expected);
        }
    }
}

#[test]
fn maximum_file_size_is_inclusive() {
    let fixture = Fixture::new();
    let smaller = fixture.write("smaller.txt", b"Rust");
    let exact = fixture.write("exact.txt", b"Rust\n");
    fixture.write("larger.txt", b"Rust\nextra\n");

    assert_selected_files(
        fixture.search_with(&["--max-filesize", "5"]),
        &[smaller, exact],
    );
}

#[test]
fn explicit_file_obeys_size_limit_at_depth_zero() {
    let fixture = Fixture::new();
    let file = fixture.write("sample.txt", b"Rust\n");

    for (limit, selected) in [("4", false), ("5", true)] {
        let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
            .args(["--max-depth", "0", "--max-filesize", limit, "-n", "Rust"])
            .arg(&file)
            .output()
            .expect("executable must start");

        if selected {
            assert_selected_files(output, std::slice::from_ref(&file));
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(output.stderr.is_empty());
        }
    }
}

#[test]
fn zero_size_limit_produces_no_selected_lines() {
    let fixture = Fixture::new();
    fixture.write("empty.txt", b"");
    fixture.write("nonempty.txt", b"Rust\n");

    let output = fixture.search_with(&["--max-filesize", "0"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn depth_and_size_limits_combine_with_existing_filters() {
    let fixture = Fixture::new();
    fixture.write(".ignore", b"ignored.txt\n");
    fixture.write("ignored.txt", b"Rust\n");
    fixture.write(".hidden.txt", b"Rust\n");
    fixture.write("nested/deep.txt", b"Rust\n");
    fixture.write("large.txt", b"Rust\nextra\n");
    let visible = fixture.write("visible.txt", b"Rust\n");

    assert_selected_files(
        fixture.search_with(&["--max-depth", "1", "--max-filesize", "5"]),
        &[visible],
    );
}

#[test]
fn invalid_depth_and_size_values_are_usage_errors() {
    let fixture = Fixture::new();

    let cases: &[&[&str]] = &[
        &["--max-depth", "abc"],
        &["--max-depth=-1"],
        &["--max-filesize", "1KB"],
        &["--max-filesize=-1"],
    ];

    for flags in cases {
        let output = fixture.search_with(flags);

        assert_eq!(output.status.code(), Some(2), "flags: {flags:?}");
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("error:"), "{stderr}");
    }
}

#[test]
fn positive_and_anchored_globs_select_paths() {
    let fixture = Fixture::new();
    let direct = fixture.write("main.rs", b"Rust\n");
    let nested = fixture.write("src/lib.rs", b"Rust\n");
    let deeper = fixture.write("src/nested/deep.rs", b"Rust\n");
    fixture.write("notes.txt", b"Rust\n");

    assert_selected_files(
        fixture.search_with(&["-g", "*.rs"]),
        &[direct, nested.clone(), deeper],
    );

    assert_selected_files(fixture.search_with(&["--glob", "/src/*.rs"]), &[nested]);
}

#[test]
fn negative_globs_leave_other_files_eligible() {
    let fixture = Fixture::new();
    fixture.write("debug.log", b"Rust\n");
    let visible = fixture.write("visible.txt", b"Rust\n");

    assert_selected_files(fixture.search_with(&["-g", "!*.log"]), &[visible]);
}

#[test]
fn later_globs_can_exclude_and_reinclude_a_file() {
    let fixture = Fixture::new();
    let handwritten = fixture.write("handwritten.rs", b"Rust\n");
    let generated = fixture.write("generated.rs", b"Rust\n");

    assert_selected_files(
        fixture.search_with(&["-g", "*.rs", "-g", "!generated.rs"]),
        std::slice::from_ref(&handwritten),
    );

    assert_selected_files(
        fixture.search_with(&["-g", "*.rs", "-g", "!generated.rs", "-g", "generated.rs"]),
        &[handwritten, generated],
    );
}

#[test]
fn positive_globs_override_ignore_rules_and_hidden_file_filtering() {
    let fixture = Fixture::new();
    fixture.write(".ignore", b"*.rs\n");

    let visible = fixture.write("visible.rs", b"Rust\n");
    let hidden = fixture.write(".hidden.rs", b"Rust\n");

    assert_selected_files(fixture.search_with(&["-g", "*.rs"]), &[visible, hidden]);
}

#[test]
fn explicit_file_roots_obey_globs() {
    let fixture = Fixture::new();
    let file = fixture.write("sample.txt", b"Rust\n");

    for (glob, selected) in [("*.rs", false), ("*.txt", true)] {
        let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
            .args(["-n", "-g", glob, "Rust"])
            .arg(&file)
            .output()
            .expect("executable must start");

        if selected {
            assert_selected_files(output, std::slice::from_ref(&file));
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(output.stderr.is_empty());
        }
    }
}

#[test]
fn glob_inclusions_do_not_bypass_depth_or_size_limits() {
    let fixture = Fixture::new();
    let visible = fixture.write("visible.rs", b"Rust\n");
    fixture.write("large.rs", b"Rust\nextra\n");
    fixture.write("nested/deep.rs", b"Rust\n");

    assert_selected_files(
        fixture.search_with(&["-g", "*.rs", "--max-depth", "1", "--max-filesize", "5"]),
        &[visible],
    );
}

#[test]
fn ignore_case_does_not_change_glob_case_sensitivity() {
    let fixture = Fixture::new();
    let lowercase = fixture.write("lower.rs", b"Rust\n");
    fixture.write("UPPER.RS", b"Rust\n");

    assert_selected_files(fixture.search_with(&["-i", "-g", "*.rs"]), &[lowercase]);
}

#[test]
fn invalid_globs_fail_before_search_output() {
    let fixture = Fixture::new();
    fixture.write("visible.rs", b"Rust\n");

    let output = fixture.search_with(&["-g", "*.rs", "-g", "[z-a]"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("[z-a]"), "{stderr}");
}

#[test]
fn excluded_directories_are_not_restored_by_child_globs() {
    let fixture = Fixture::new();
    let visible = fixture.write("visible.rs", b"Rust\n");
    fixture.write("vendor/keep.rs", b"Rust\n");

    assert_selected_files(
        fixture.search_with(&["-g", "!vendor/", "-g", "vendor/keep.rs", "-g", "*.rs"]),
        &[visible],
    );
}

#[test]
fn repeated_type_inclusions_select_multiple_types() {
    let fixture = Fixture::new();
    let rust = fixture.write("nested/main.rs", b"Rust\n");
    let toml = fixture.write("Cargo.toml", b"Rust\n");
    let lock = fixture.write("Cargo.lock", b"Rust\n");
    fixture.write("notes.txt", b"Rust\n");

    assert_selected_files(
        fixture.search_with(&["--type", "rust", "--type", "toml"]),
        &[rust, toml, lock],
    );
}

#[test]
fn type_exclusions_win_in_either_argument_order() {
    let fixture = Fixture::new();
    fixture.write("main.rs", b"Rust\n");
    let text = fixture.write("notes.txt", b"Rust\n");

    let cases: &[&[&str]] = &[
        &["--type", "rust", "--type-not", "rust"],
        &["--type-not", "rust", "--type", "rust"],
    ];

    for flags in cases {
        let output = fixture.search_with(flags);

        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    assert_selected_files(fixture.search_with(&["--type-not", "rust"]), &[text]);
}

#[test]
fn type_filters_respect_ignore_rules_and_constrain_glob_inclusions() {
    let fixture = Fixture::new();
    fixture.write(".ignore", b"ignored.rs\n");
    fixture.write("ignored.rs", b"Rust\n");
    fixture.write(".hidden.rs", b"Rust\n");
    let visible = fixture.write("visible.rs", b"Rust\n");

    assert_selected_files(fixture.search_with(&["--type", "rust"]), &[visible]);

    let output = fixture.search_with(&["-g", "*.rs", "--type-not", "rust"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn explicit_files_obey_type_filters() {
    let fixture = Fixture::new();
    let file = fixture.write("Cargo.toml", b"Rust\n");

    for (file_type, selected) in [("rust", false), ("toml", true)] {
        let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
            .args(["-n", "--type", file_type, "Rust"])
            .arg(&file)
            .output()
            .expect("executable must start");

        if selected {
            assert_selected_files(output, std::slice::from_ref(&file));
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(output.stderr.is_empty());
        }
    }
}

#[test]
fn unknown_types_fail_before_search_output() {
    let fixture = Fixture::new();
    fixture.write("visible.rs", b"Rust\n");

    for flag in ["--type", "--type-not"] {
        let output = fixture.search_with(&[flag, "mini-grep-unknown-type"]);

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("mini-grep-unknown-type"), "{stderr}");
    }
}

#[cfg(any(unix, windows))]
mod symlink_tests {
    use super::*;
    use std::path::Path;

    fn file_link(target: &Path, link: &Path) {
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(target, link);

        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_file(target, link);

        result.expect("file symlink creation must be available");
    }

    fn directory_link(target: &Path, link: &Path) {
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(target, link);

        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_dir(target, link);

        result.expect("directory symlink creation must be available");
    }

    #[test]
    fn discovered_symlinks_are_skipped_by_default() {
        let fixture = Fixture::new();
        let file = fixture.write("target.txt", b"Rust\n");
        let child = fixture.write("real/child.txt", b"Rust\n");

        file_link(&file, &fixture.root.join("file-link.txt"));
        directory_link(
            child.parent().expect("child must have a parent"),
            &fixture.root.join("directory-link"),
        );

        assert_selected_files(fixture.search(), &[file, child]);
    }

    #[test]
    fn follow_searches_file_and_directory_aliases() {
        let fixture = Fixture::new();
        let file = fixture.write("target.txt", b"Rust\n");
        let child = fixture.write("real/child.txt", b"Rust\n");

        let file_alias = fixture.root.join("file-link.txt");
        let directory_alias = fixture.root.join("directory-link");

        file_link(&file, &file_alias);
        directory_link(
            child.parent().expect("child must have a parent"),
            &directory_alias,
        );

        assert_selected_files(
            fixture.search_with(&["-L"]),
            &[file, child, file_alias, directory_alias.join("child.txt")],
        );
    }

    #[test]
    fn explicit_symlink_roots_require_follow() {
        let fixture = Fixture::new();
        let file = fixture.write("target.txt", b"Rust\n");
        let child = fixture.write("real/child.txt", b"Rust\n");

        let file_alias = fixture.root.join("file-link.txt");
        let directory_alias = fixture.root.join("directory-link");

        file_link(&file, &file_alias);
        directory_link(
            child.parent().expect("child must have a parent"),
            &directory_alias,
        );

        for link in [&file_alias, &directory_alias] {
            let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
                .args(["-n", "Rust"])
                .arg(link)
                .output()
                .expect("executable must start");

            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());

            let stderr = String::from_utf8_lossy(&output.stderr);
            let displayed_path = link.display().to_string();

            assert!(stderr.contains("--follow"), "{stderr}");
            assert!(stderr.contains(displayed_path.as_str()), "{stderr}");
        }
    }

    #[test]
    fn follow_accepts_explicit_file_and_directory_symlink_roots() {
        let fixture = Fixture::new();
        let file = fixture.write("target.txt", b"Rust\n");
        let child = fixture.write("real/child.txt", b"Rust\n");

        let file_alias = fixture.root.join("file-link.txt");
        let directory_alias = fixture.root.join("directory-link");

        file_link(&file, &file_alias);
        directory_link(
            child.parent().expect("child must have a parent"),
            &directory_alias,
        );

        let cases = [
            (file_alias.clone(), file_alias),
            (directory_alias.clone(), directory_alias.join("child.txt")),
        ];

        for (root, expected) in cases {
            let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
                .args(["--follow", "-n", "Rust"])
                .arg(root)
                .output()
                .expect("executable must start");

            assert_selected_files(output, &[expected]);
        }
    }

    #[test]
    fn directory_cycles_are_reported_without_losing_matches() {
        let fixture = Fixture::new();
        let visible = fixture.write("visible.txt", b"Rust\n");
        let cycle = fixture.root.join("cycle");

        directory_link(&fixture.root, &cycle);

        let output = fixture.search_with(&["--follow"]);

        assert_eq!(output.status.code(), Some(2));

        let expected = format!("{}:1:Rust\n", visible.display());
        assert_eq!(output.stdout, expected.as_bytes());

        let stderr = String::from_utf8_lossy(&output.stderr);
        let cycle_path = cycle.display().to_string();

        assert!(stderr.to_lowercase().contains("loop"), "{stderr}");
        assert!(stderr.contains(cycle_path.as_str()), "{stderr}");
    }

    #[test]
    fn broken_links_are_skipped_or_reported_according_to_policy() {
        let fixture = Fixture::new();
        let visible = fixture.write("visible.txt", b"Rust\n");
        let broken = fixture.root.join("broken.txt");

        file_link(&fixture.root.join("absent.txt"), &broken);

        assert_selected_files(fixture.search(), std::slice::from_ref(&visible));

        let output = fixture.search_with(&["--follow"]);

        assert_eq!(output.status.code(), Some(2));

        let expected = format!("{}:1:Rust\n", visible.display());
        assert_eq!(output.stdout, expected.as_bytes());

        let stderr = String::from_utf8_lossy(&output.stderr);
        let broken_path = broken.display().to_string();

        assert!(stderr.contains("cannot discover files"), "{stderr}");
        assert!(stderr.contains(broken_path.as_str()), "{stderr}");
    }

    #[test]
    fn followed_file_aliases_obey_path_and_size_filters() {
        let fixture = Fixture::new();
        let target = fixture.write("target.txt", b"Rust\n");
        let alias = fixture.root.join("alias.rs");

        file_link(&target, &alias);

        assert_selected_files(
            fixture.search_with(&[
                "--follow",
                "--type",
                "rust",
                "-g",
                "*.rs",
                "--max-filesize",
                "5",
            ]),
            std::slice::from_ref(&alias),
        );

        let output = fixture.search_with(&[
            "--follow",
            "--type",
            "rust",
            "-g",
            "*.rs",
            "--max-filesize",
            "4",
        ]);

        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn locked_files_do_not_discard_other_files_results() {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;

        let fixture = Fixture::new();
        let locked = fixture.write("locked.txt", b"Rust\n");
        let visible = fixture.write("visible.txt", b"Rust\n");

        let lock = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&locked)
            .expect("exclusive file handle must open");

        let output = fixture.search();

        drop(lock);

        assert_eq!(output.status.code(), Some(2));

        let expected = format!("{}:1:Rust\n", visible.display());
        assert_eq!(output.stdout, expected.as_bytes());

        let stderr = String::from_utf8_lossy(&output.stderr);
        let locked_path = locked.display().to_string();

        assert!(stderr.contains("cannot open"), "{stderr}");
        assert!(stderr.contains(locked_path.as_str()), "{stderr}");
    }
}

#[test]
fn context_state_resets_between_files() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"before first\nRust\n");
    let second = fixture.write(
        "nested/second.txt",
        b"omitted\nbefore second\nRust\nafter second\nomitted\n",
    );
    fixture.write("no-match.txt", b"never emitted\n");

    let output = fixture.search_with(&["-C", "1"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let first_group = format!(
        "{}-1-before first\n{}:2:Rust\n",
        first.display(),
        first.display(),
    );
    let second_group = format!(
        "{}-2-before second\n{}:3:Rust\n{}-4-after second\n",
        second.display(),
        second.display(),
        second.display(),
    );

    let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");

    assert!(
        stdout == format!("{first_group}{second_group}")
            || stdout == format!("{second_group}{first_group}"),
        "unexpected output: {stdout:?}"
    );
}

#[test]
fn global_limit_is_shared_while_per_file_limits_reset() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"Rust\nRust\nRust\n");
    let second = fixture.write("second.txt", b"Rust\nRust\nRust\n");

    let output = fixture.search_with(&["-m", "2", "--max-results", "3"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let first = first.display();
    let second = second.display();

    let first_then_second = format!(
        "{first}:1:Rust\n\
        {first}:2:Rust\n\
        {second}:1:Rust\n"
    );
    let second_then_first = format!(
        "{second}:1:Rust\n\
        {second}:2:Rust\n\
        {first}:1:Rust\n"
    );

    let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");

    assert!(
        stdout == first_then_second || stdout == second_then_first,
        "unexpected output: {stdout:?}"
    );
}

#[test]
fn global_limit_preserves_trailing_read_errors_and_stops_other_files() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"Rust\n\xff\n");
    let second = fixture.write("second.txt", b"Rust\n\xff\n");

    let output = fixture.search_with(&["--max-results", "1", "-A", "1"]);

    assert_eq!(output.status.code(), Some(2));

    let stdout =
        String::from_utf8(output.stdout).expect("printed fixture text must be valid UTF-8");

    let first_output = format!("{}:1:Rust\n", first.display());
    let second_output = format!("{}:1:Rust\n", second.display());

    let (searched, unsearched) = if stdout == first_output {
        (&first, &second)
    } else {
        assert_eq!(stdout, second_output);
        (&second, &first)
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    let searched_path = searched.display().to_string();
    let unsearched_path = unsearched.display().to_string();

    assert!(stderr.contains("cannot read"), "{stderr}");
    assert!(stderr.contains(searched_path.as_str()), "{stderr}");
    assert!(!stderr.contains(unsearched_path.as_str()), "{stderr}");
    assert!(!stderr.contains("panicked at"), "{stderr}");
}

#[test]
fn quiet_selection_preserves_an_earlier_discovery_error() {
    let fixture = Fixture::new();
    let ignore_file = fixture.write(".ignore", b"\xff\n");
    let target = fixture.write("target.txt", b"Rust\n\xff\n");

    let output = fixture.search_with(&["-q"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    let ignore_path = ignore_file.display().to_string();
    let target_path = target.display().to_string();

    assert!(stderr.contains(ignore_path.as_str()), "{stderr}");
    assert!(!stderr.contains(target_path.as_str()), "{stderr}");
    assert!(!stderr.contains("panicked at"), "{stderr}");

    let without_ignore = fixture.search_with(&["-q", "--no-ignore"]);

    assert_eq!(without_ignore.status.code(), Some(0));
    assert!(without_ignore.stdout.is_empty());
    assert!(
        without_ignore.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&without_ignore.stderr)
    );
}

#[test]
fn file_list_modes_report_qualifying_files_across_a_directory() {
    let fixture = Fixture::new();
    let matching = fixture.write("matching.txt", b"Rust\nRust\n");
    let nonmatching = fixture.write("nested/nonmatching.txt", b"Go\n");
    let empty = fixture.write("empty.txt", b"");

    let cases = [
        ("-l", vec![matching]),
        ("--files-without-match", vec![nonmatching, empty]),
    ];

    for (flag, paths) in cases {
        let output = fixture.search_with(&[flag, "-C", "2"]);

        assert_eq!(output.status.code(), Some(0));
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");
        let mut actual: Vec<_> = stdout.lines().map(str::to_owned).collect();
        let mut expected: Vec<_> = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect();

        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);

        let quiet = fixture.search_with(&[flag, "-q"]);

        assert_eq!(quiet.status.code(), Some(0));
        assert!(quiet.stdout.is_empty());
        assert!(
            quiet.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&quiet.stderr)
        );
    }
}

#[test]
fn file_list_modes_obey_zero_and_global_selection_limits() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"Rust\n\xff\n");
    let second = fixture.write("second.txt", b"Rust\n\xff\n");

    for flag in ["-l", "--files-without-match"] {
        for limit_flag in ["-m", "--max-results"] {
            let output = fixture.search_with(&[flag, limit_flag, "0"]);

            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(
                output.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let output = fixture.search_with(&[flag, "--max-results", "1"]);

        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        if flag == "-l" {
            assert_eq!(output.status.code(), Some(0));

            let first_path = format!("{}\n", first.display());
            let second_path = format!("{}\n", second.display());

            assert!(
                output.stdout == first_path.as_bytes() || output.stdout == second_path.as_bytes(),
                "unexpected output: {:?}",
                String::from_utf8_lossy(&output.stdout)
            );
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn count_modes_preserve_completed_records_when_another_file_fails() {
    let fixture = Fixture::new();
    let matching = fixture.write("matching.txt", b"Rust Rust\nRust\n");
    let nonmatching = fixture.write("nonmatching.txt", b"Go\n");
    let empty = fixture.write("empty.txt", b"");
    let broken = fixture.write("broken.txt", b"Rust\n\xff\n");

    for (mode, matching_count) in [("-c", 2), ("--count-matches", 3)] {
        let output = fixture.search_with(&[mode]);

        assert_eq!(output.status.code(), Some(2));

        let stderr = String::from_utf8_lossy(&output.stderr);
        let broken_path = broken.display().to_string();
        assert!(stderr.contains("cannot read"), "{stderr}");
        assert!(stderr.contains(broken_path.as_str()), "{stderr}");

        let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");
        let mut actual: Vec<_> = stdout.lines().map(str::to_owned).collect();
        let mut expected = vec![
            format!("{}:{matching_count}", matching.display()),
            format!("{}:0", nonmatching.display()),
            format!("{}:0", empty.display()),
        ];

        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }
}

#[test]
fn count_modes_share_global_limits_and_reset_per_file_limits() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"Rust Rust\nRust\nRust\n");
    let second = fixture.write("second.txt", b"Rust Rust\nRust\nRust\n");

    for (mode, full_count, partial_count) in [("-c", 2, 1), ("--count-matches", 3, 2)] {
        let output = fixture.search_with(&[mode, "-m", "2", "--max-results", "3"]);

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");
        let mut actual: Vec<_> = stdout.lines().map(str::to_owned).collect();

        let mut first_then_second = vec![
            format!("{}:{full_count}", first.display()),
            format!("{}:{partial_count}", second.display()),
        ];
        let mut second_then_first = vec![
            format!("{}:{partial_count}", first.display()),
            format!("{}:{full_count}", second.display()),
        ];

        actual.sort();
        first_then_second.sort();
        second_then_first.sort();

        assert!(
            actual == first_then_second || actual == second_then_first,
            "unexpected records: {actual:?}"
        );
    }
}

#[test]
fn match_only_output_shares_global_limits_and_resets_per_file_limits() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"Rust Rust\nRust\n\xff\n");
    let second = fixture.write("second.txt", b"Rust Rust\nRust\n\xff\n");

    let output = fixture.search_with(&["-o", "-m", "2", "--max-results", "3", "-A", "3"]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let records = |path: &PathBuf, includes_second_line: bool| {
        let path = path.display();
        let mut text = format!("{path}:1:Rust\n{path}:1:Rust\n");

        if includes_second_line {
            text.push_str(&format!("{path}:2:Rust\n"));
        }

        text
    };

    let first_then_second = format!("{}{}", records(&first, true), records(&second, false),);
    let second_then_first = format!("{}{}", records(&second, true), records(&first, false),);

    let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");

    assert!(
        stdout == first_then_second || stdout == second_then_first,
        "unexpected output: {stdout:?}"
    );
}

#[test]
fn headings_separate_visible_files_without_groups_for_empty_results() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"Rust\nRust\n");
    let second = fixture.write("nested/second.txt", b"Go\nRust\n");
    fixture.write("no-match.txt", b"Go\n");
    fixture.write("empty.txt", b"");

    for flags in [vec!["--heading"], vec!["--heading", "-o"]] {
        let output = fixture.search_with(&flags);

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let first_group = format!("{}\n1:Rust\n2:Rust\n", first.display());
        let second_group = format!("{}\n2:Rust\n", second.display());

        let stdout = String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");

        assert!(
            stdout == format!("{first_group}\n{second_group}")
                || stdout == format!("{second_group}\n{first_group}"),
            "unexpected output: {stdout:?}"
        );
    }
}

#[test]
fn null_file_lists_emit_complete_paths_without_newline_delimiters() {
    let fixture = Fixture::new();
    let first = fixture.write("first file.txt", b"Rust\n");
    let second = fixture.write("nested/second.txt", b"Rust\n");
    let nonmatching = fixture.write("other.txt", b"Go\n");
    let empty = fixture.write("empty.txt", b"");

    let cases = [
        ("-l", vec![first, second]),
        ("--files-without-match", vec![nonmatching, empty]),
    ];

    for (mode, paths) in cases {
        let output = fixture.search_with(&["-0", mode]);

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let mut actual: Vec<Vec<u8>> = output
            .stdout
            .split(|byte| *byte == 0)
            .map(<[u8]>::to_vec)
            .collect();

        assert_eq!(actual.pop(), Some(Vec::new()));

        let mut expected: Vec<Vec<u8>> = paths
            .iter()
            .map(|path| {
                path.to_str()
                    .expect("fixture paths must be valid Unicode")
                    .as_bytes()
                    .to_vec()
            })
            .collect();

        actual.sort();
        expected.sort();

        assert_eq!(actual, expected);
    }
}

#[test]
fn stdin_patterns_default_to_searching_the_current_directory() {
    use std::io::Write;
    use std::process::Stdio;

    let fixture = Fixture::new();
    fixture.write("target.txt", b"Rust\nGo\n");
    fixture.write("other.txt", b"Python\n");

    let mut child = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .current_dir(&fixture.root)
        .args(["-n", "-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executable must start");

    {
        let mut input = child.stdin.take().expect("child stdin must be available");
        input
            .write_all(b"Rust\r\n")
            .expect("patterns must be written");
    }

    let output = child
        .wait_with_output()
        .expect("child output must be collected");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let target = std::path::Path::new(".").join("target.txt");
    let expected = format!("{}:1:Rust\n", target.display());

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn file_enumeration_lists_candidates_without_reading_contents() {
    let fixture = Fixture::new();
    let text = fixture.write("text.txt", b"Go\n");
    let empty = fixture.write("empty.txt", b"");
    let invalid = fixture.write("nested/bytes.bin", b"\xff\n");

    let output = fixture.list_files(&[]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let stdout =
        String::from_utf8(output.stdout).expect("fixture paths must produce valid UTF-8 output");
    let mut actual: Vec<_> = stdout.lines().map(str::to_owned).collect();
    let mut expected = vec![
        text.display().to_string(),
        empty.display().to_string(),
        invalid.display().to_string(),
    ];

    actual.sort();
    expected.sort();

    assert_eq!(actual, expected);
}

#[test]
fn empty_file_enumeration_returns_one() {
    let fixture = Fixture::new();

    for flags in [vec![], vec!["-q"]] {
        let output = fixture.list_files(&flags);

        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn file_enumeration_reuses_discovery_filters() {
    let fixture = Fixture::new();
    fixture.write(".ignore", b"ignored.txt\n");

    let visible = fixture.write("visible.txt", b"1234");
    let nested = fixture.write("nested/deep.txt", b"12345");
    let rust = fixture.write("code.rs", b"Rust");
    let hidden = fixture.write(".hidden.txt", b"1234");
    let ignored = fixture.write("ignored.txt", b"1234");

    let cases = [
        (vec![], vec![&visible, &nested, &rust]),
        (vec!["--hidden"], vec![&visible, &nested, &rust, &hidden]),
        (
            vec!["--no-ignore"],
            vec![&visible, &nested, &rust, &ignored],
        ),
        (vec!["--max-depth", "1"], vec![&visible, &rust]),
        (vec!["--max-filesize", "4"], vec![&visible, &rust]),
        (vec!["--type", "rust"], vec![&rust]),
        (vec!["--type-not", "rust"], vec![&visible, &nested]),
        (
            vec!["-g", "*.txt", "-g", "!ignored.txt"],
            vec![&visible, &nested, &hidden],
        ),
    ];

    for (extra_flags, paths) in cases {
        // Keep the ignore-rule file out of enumeration results.
        let mut flags = vec!["-g", "!.ignore"];
        flags.extend(extra_flags);

        let output = fixture.list_files(&flags);

        assert_eq!(
            output.status.code(),
            Some(0),
            "flags: {flags:?}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let stdout = String::from_utf8(output.stdout)
            .expect("fixture paths must produce valid UTF-8 output");
        let mut actual: Vec<_> = stdout.lines().map(str::to_owned).collect();
        let mut expected: Vec<_> = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect();

        actual.sort();
        expected.sort();

        assert_eq!(actual, expected, "flags: {flags:?}");
    }
}

#[test]
fn file_enumeration_supports_null_paths_and_quiet_mode() {
    let fixture = Fixture::new();
    let first = fixture.write("first file.txt", b"");
    let second = fixture.write("nested/café.txt", b"");

    for flags in [vec!["-0"], vec!["--null", "-h", "--column"]] {
        let output = fixture.list_files(&flags);

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let mut actual: Vec<Vec<u8>> = output
            .stdout
            .split(|byte| *byte == 0)
            .map(<[u8]>::to_vec)
            .collect();

        assert_eq!(actual.pop(), Some(Vec::new()));

        let mut expected: Vec<Vec<u8>> = [&first, &second]
            .iter()
            .map(|path| {
                path.to_str()
                    .expect("fixture paths must be valid Unicode")
                    .as_bytes()
                    .to_vec()
            })
            .collect();

        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }

    let quiet = fixture.list_files(&["-q", "-0", "-h", "--column"]);

    assert_eq!(quiet.status.code(), Some(0));
    assert!(quiet.stdout.is_empty());
    assert!(quiet.stderr.is_empty());
}

#[test]
fn file_enumeration_preserves_errors_before_quiet_stopping() {
    let fixture = Fixture::new();
    let ignore_file = fixture.write(".ignore", b"\xff\n");
    let target = fixture.write("target.txt", b"\xff\n");

    for quiet in [false, true] {
        let flags = if quiet { vec!["-q"] } else { vec![] };
        let output = fixture.list_files(&flags);

        assert_eq!(output.status.code(), Some(2));

        let expected = if quiet {
            String::new()
        } else {
            format!("{}\n", target.display())
        };

        assert_eq!(output.stdout, expected.as_bytes());

        let stderr = String::from_utf8_lossy(&output.stderr);
        let ignore_path = ignore_file.display().to_string();
        let target_path = target.display().to_string();

        assert!(stderr.contains(ignore_path.as_str()), "{stderr}");
        assert!(!stderr.contains(target_path.as_str()), "{stderr}");
        assert!(!stderr.contains("panicked at"), "{stderr}");
    }
}

#[test]
fn file_enumeration_defaults_to_current_directory_despite_piped_stdin() {
    use std::io::Write;
    use std::process::Stdio;

    let fixture = Fixture::new();
    fixture.write("target.txt", b"");

    let mut child = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .current_dir(&fixture.root)
        .arg("--files")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executable must start");

    {
        let mut input = child.stdin.take().expect("child stdin must be available");

        if let Err(error) = input.write_all(b"ignored stdin contents\n") {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe,
                "unexpected stdin write error: {error}"
            );
        }
    }

    let output = child
        .wait_with_output()
        .expect("child output must be collected");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let path = std::path::Path::new(".").join("target.txt");
    let expected = format!("{}\n", path.display());

    assert_eq!(output.stdout, expected.as_bytes());
}

#[cfg(windows)]
#[test]
fn file_enumeration_lists_exclusively_locked_files() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    let fixture = Fixture::new();
    let target = fixture.write("locked.txt", b"\xff\n");

    let lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&target)
        .expect("exclusive fixture handle must open");

    let cases: &[&[&str]] = &[&[], &["-n", "--column", "--heading", "-H"], &["-h"]];

    for flags in cases {
        let output = fixture.list_files(flags);

        assert_eq!(
            output.status.code(),
            Some(0),
            "flags: {flags:?}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let expected = format!("{}\n", target.display());
        assert_eq!(output.stdout, expected.as_bytes());
    }

    drop(lock);
}

#[test]
fn path_list_enumeration_preserves_root_order_and_duplicates() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"");
    let second = fixture.write("second.txt", b"");

    let contents = format!(
        "{}\r\n{}\n{}\n",
        first.display(),
        second.display(),
        first.display(),
    );
    let list = fixture.write("targets.paths", contents.as_bytes());

    let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .args(["--files", "--files-from"])
        .arg(&list)
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
        "{}\n{}\n{}\n",
        first.display(),
        second.display(),
        first.display(),
    );

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn empty_path_list_enumeration_does_not_use_a_default_root() {
    let fixture = Fixture::new();
    fixture.write("would-be-listed.txt", b"");
    let list = fixture.write("empty.paths", b"");

    let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .current_dir(&fixture.root)
        .args(["--files", "--files-from"])
        .arg(&list)
        .output()
        .expect("executable must start");

    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn listed_search_roots_share_limits_in_source_order() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"Rust Rust\nRust\n\xff\n");
    let second = fixture.write("second.txt", b"Rust Rust\nRust\n\xff\n");

    let contents = format!("{}\n{}\n", first.display(), second.display(),);
    let list = fixture.write("targets.paths", contents.as_bytes());

    for only_matching in [false, true] {
        let mut process = Command::new(env!("CARGO_BIN_EXE_mini-grep"));
        process.args(["-n", "-m", "2", "--max-results", "3"]);

        if only_matching {
            process.arg("-o");
        }

        let output = process
            .args(["Rust", "--files-from"])
            .arg(&list)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let first = first.display();
        let second = second.display();

        let expected = if only_matching {
            format!(
                "{first}:1:Rust\n\
                {first}:1:Rust\n\
                {first}:2:Rust\n\
                {second}:1:Rust\n\
                {second}:1:Rust\n"
            )
        } else {
            format!(
                "{first}:1:Rust Rust\n\
                {first}:2:Rust\n\
                {second}:1:Rust Rust\n"
            )
        };

        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn listed_paths_resolve_from_current_directory_and_dash_is_a_filename() {
    let fixture = Fixture::new();
    fixture.write("target.txt", b"Rust\n");
    fixture.write("-", b"Rust\n");
    let list = fixture.write("lists/targets.paths", b"target.txt\n-\n");

    for enumerate in [false, true] {
        let mut process = Command::new(env!("CARGO_BIN_EXE_mini-grep"));
        process.current_dir(&fixture.root);

        if enumerate {
            process.arg("--files");
        } else {
            process.args(["-n", "Rust"]);
        }

        let output = process
            .arg("--files-from")
            .arg(&list)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let expected: &[u8] = if enumerate {
            b"target.txt\n-\n"
        } else {
            b"target.txt:1:Rust\n-:1:Rust\n"
        };

        assert_eq!(output.stdout, expected);
    }
}

#[test]
fn empty_listed_search_does_not_use_stdin_or_current_directory() {
    use std::io::Write;
    use std::process::Stdio;

    let fixture = Fixture::new();
    fixture.write("target.txt", b"Rust\n");
    let list = fixture.write("empty.paths", b"");

    let mut child = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .current_dir(&fixture.root)
        .args(["Rust", "--files-from"])
        .arg(&list)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executable must start");

    {
        let mut input = child.stdin.take().expect("child stdin must be available");

        if let Err(error) = input.write_all(b"Rust\n") {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe,
                "unexpected stdin write error: {error}"
            );
        }
    }

    let output = child
        .wait_with_output()
        .expect("child output must be collected");

    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn listed_target_errors_preserve_results_and_continue_to_later_roots() {
    let fixture = Fixture::new();
    let missing = fixture.root.join("missing.txt");
    let broken = fixture.write("broken.txt", b"Rust\n\xff\n");
    let good = fixture.write("good.txt", b"Rust\n");

    let contents = format!(
        "{}\n{}\n{}\n",
        missing.display(),
        broken.display(),
        good.display(),
    );
    let list = fixture.write("targets.paths", contents.as_bytes());

    for enumerate in [false, true] {
        let mut process = Command::new(env!("CARGO_BIN_EXE_mini-grep"));

        if enumerate {
            process.arg("--files");
        } else {
            process.args(["-n", "Rust"]);
        }

        let output = process
            .arg("--files-from")
            .arg(&list)
            .output()
            .expect("executable must start");

        assert_eq!(output.status.code(), Some(2));

        let expected = if enumerate {
            format!("{}\n{}\n", broken.display(), good.display())
        } else {
            format!("{}:1:Rust\n{}:1:Rust\n", broken.display(), good.display(),)
        };

        assert_eq!(output.stdout, expected.as_bytes());

        let stderr = String::from_utf8_lossy(&output.stderr);
        let missing_path = missing.display().to_string();
        assert!(stderr.contains(missing_path.as_str()), "{stderr}");

        if !enumerate {
            let broken_path = broken.display().to_string();
            assert!(stderr.contains(broken_path.as_str()), "{stderr}");
            assert!(stderr.contains("cannot read"), "{stderr}");
        }
    }
}

#[test]
fn listed_quiet_search_preserves_earlier_errors_and_skips_later_roots() {
    let fixture = Fixture::new();
    let early = fixture.root.join("early-missing.txt");
    let good = fixture.write("good.txt", b"Rust\n");
    let late = fixture.root.join("late-missing.txt");

    let contents = format!(
        "{}\n{}\n{}\n",
        early.display(),
        good.display(),
        late.display(),
    );
    let list = fixture.write("targets.paths", contents.as_bytes());

    for enumerate in [false, true] {
        let mut process = Command::new(env!("CARGO_BIN_EXE_mini-grep"));
        process.arg("-q");

        if enumerate {
            process.arg("--files");
        } else {
            process.arg("Rust");
        }

        let output = process
            .arg("--files-from")
            .arg(&list)
            .output()
            .expect("executable must start");

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        let early_path = early.display().to_string();
        let late_path = late.display().to_string();

        assert!(stderr.contains(early_path.as_str()), "{stderr}");
        assert!(!stderr.contains(late_path.as_str()), "{stderr}");
    }
}

#[test]
fn listed_directory_roots_reuse_discovery_filters() {
    let fixture = Fixture::new();
    let first = fixture.write("first/src/lib.rs", b"Rust");
    let second = fixture.write("second/src/main.rs", b"Rust");

    fixture.write("first/.ignore", b"src/ignored.rs\n");
    fixture.write("first/src/ignored.rs", b"Rust");
    fixture.write("first/src/.hidden.rs", b"Rust");
    fixture.write("first/src/notes.txt", b"Rust");
    fixture.write("first/src/big.rs", b"RustRust");
    fixture.write("second/src/nested/deep.rs", b"Rust");
    fixture.write("second/src/skip.rs", b"Rust");

    let contents = format!(
        "{}\n{}\n",
        fixture.root.join("first").display(),
        fixture.root.join("second").display(),
    );
    let list = fixture.write("targets.paths", contents.as_bytes());

    for enumerate in [false, true] {
        let mut process = Command::new(env!("CARGO_BIN_EXE_mini-grep"));
        process.args([
            "--type",
            "rust",
            "--max-filesize",
            "4",
            "--max-depth",
            "2",
            "-g",
            "!skip.rs",
        ]);

        if enumerate {
            process.arg("--files");
        } else {
            process.args(["-n", "Rust"]);
        }

        let output = process
            .arg("--files-from")
            .arg(&list)
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let expected = if enumerate {
            format!("{}\n{}\n", first.display(), second.display())
        } else {
            format!("{}:1:Rust\n{}:1:Rust\n", first.display(), second.display(),)
        };

        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn listed_dash_directories_are_filesystem_roots() {
    let fixture = Fixture::new();
    fixture.write("-/target.txt", b"Rust\n");

    for spelling in ["-/", "-/.", "-//"] {
        let contents = format!("{spelling}\n");
        let list = fixture.write("targets.paths", contents.as_bytes());

        for enumerate in [false, true] {
            let mut process = Command::new(env!("CARGO_BIN_EXE_mini-grep"));
            process.current_dir(&fixture.root);

            if enumerate {
                process.arg("--files");
            } else {
                process.args(["-n", "Rust"]);
            }

            let output = process
                .arg("--files-from")
                .arg(&list)
                .output()
                .expect("executable must start");

            assert_eq!(
                output.status.code(),
                Some(0),
                "root: {spelling}, stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());

            let stdout =
                String::from_utf8(output.stdout).expect("fixture output must be valid UTF-8");
            let record = stdout
                .strip_suffix('\n')
                .expect("one output record must end with a newline");

            let path = if enumerate {
                record
            } else {
                record
                    .strip_suffix(":1:Rust")
                    .expect("search must report the selected line")
            };

            assert_eq!(
                std::path::Path::new(path),
                std::path::Path::new("-").join("target.txt"),
            );
        }
    }
}

#[test]
fn binary_inputs_do_not_block_later_roots_or_file_enumeration() {
    let fixture = Fixture::new();
    fixture.write("binary.txt", b"\0Rust\n");
    fixture.write("good.txt", b"Rust\n");
    fixture.write("targets.paths", b"binary.txt\ngood.txt\n");

    let cases: &[(&[&str], &[u8])] = &[
        (&["-n", "Rust"], b"good.txt:1:Rust\n"),
        (&["-c", "Rust"], b"good.txt:1\n"),
        (&["--files"], b"binary.txt\ngood.txt\n"),
    ];

    for &(arguments, expected) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
            .current_dir(&fixture.root)
            .args(arguments)
            .args(["--files-from", "targets.paths"])
            .output()
            .expect("executable must start");

        assert_eq!(
            output.status.code(),
            Some(0),
            "arguments: {arguments:?}; stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected, "arguments: {arguments:?}");
    }
}
