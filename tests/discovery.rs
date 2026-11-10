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
