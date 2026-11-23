use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    directory: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after the Unix epoch")
            .as_nanos();
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "mini-grep-config-cli-{}-{timestamp}-{id}",
            std::process::id()
        ));

        fs::create_dir(&directory).expect("fixture directory must be created");
        fs::write(directory.join("sample.txt"), b"Rust\n").expect("sample file must be written");

        Self { directory }
    }

    fn write_config(&self, contents: &[u8]) {
        fs::write(self.directory.join(".mini-grep.json"), contents)
            .expect("configuration fixture must be written");
    }

    fn run(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_mini-grep"))
            .current_dir(&self.directory)
            .args(arguments)
            .stdin(Stdio::null())
            .output()
            .expect("executable must start")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn missing_local_configuration_uses_builtin_defaults() {
    let fixture = Fixture::new();
    let output = fixture.run(&["-h", "Rust", "sample.txt"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"Rust\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn local_configuration_controls_color_when_cli_omits_it() {
    let fixture = Fixture::new();
    fixture.write_config(br#"{"color":"always"}"#);

    let output = fixture.run(&["-h", "Rust", "sample.txt"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.contains(&0x1b));
}

#[test]
fn explicit_color_values_override_local_configuration() {
    let fixture = Fixture::new();
    fixture.write_config(br#"{"color":"always"}"#);

    for color in ["--color=never", "--color=auto"] {
        let output = fixture.run(&[color, "-h", "Rust", "sample.txt"]);

        assert_eq!(output.status.code(), Some(0), "option: {color}");
        assert_eq!(output.stdout, b"Rust\n", "option: {color}");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn invalid_local_configuration_fails_unless_loading_is_disabled() {
    let fixture = Fixture::new();

    for contents in [
        b"{".as_slice(),
        br#"{"threads":0}"#.as_slice(),
        br#"{"unknown":true}"#.as_slice(),
        b"\xff".as_slice(),
    ] {
        fixture.write_config(contents);

        let output = fixture.run(&["-h", "Rust", "sample.txt"]);

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8(output.stderr).expect("diagnostic must be UTF-8");
        assert!(stderr.contains(".mini-grep.json"), "{stderr}");

        let bypassed = fixture.run(&["--no-config", "-h", "Rust", "sample.txt"]);

        assert_eq!(bypassed.status.code(), Some(0));
        assert_eq!(bypassed.stdout, b"Rust\n");
        assert!(bypassed.stderr.is_empty());
    }
}

#[test]
fn help_and_version_do_not_load_invalid_configuration() {
    let fixture = Fixture::new();
    fixture.write_config(b"{");

    for option in ["--help", "--version"] {
        let output = fixture.run(&[option]);

        assert_eq!(output.status.code(), Some(0), "option: {option}");
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn configuration_is_not_loaded_from_parent_directories() {
    let fixture = Fixture::new();
    fixture.write_config(b"{");

    let child = fixture.directory.join("child");
    fs::create_dir(&child).expect("child directory must be created");
    fs::write(child.join("sample.txt"), b"Rust\n").expect("child sample must be written");

    let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .current_dir(&child)
        .args(["-h", "Rust", "sample.txt"])
        .stdin(Stdio::null())
        .output()
        .expect("executable must start");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"Rust\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn config_show_prints_resolved_local_defaults() {
    let fixture = Fixture::new();
    fixture.write_config(br#"{"threads":4,"color":"never","sort":"path"}"#);

    let output = fixture.run(&["config", "show"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));

    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("configuration output must be valid JSON");

    assert_eq!(
        document,
        serde_json::json!({
            "threads": 4,
            "color": "never",
            "sort": "path"
        })
    );
}

#[test]
fn config_show_can_bypass_invalid_local_configuration() {
    let fixture = Fixture::new();
    fixture.write_config(b"{");

    let output = fixture.run(&["config", "show", "--no-config"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("configuration output must be valid JSON");

    assert_eq!(
        document,
        serde_json::json!({
            "threads": 1,
            "color": "auto",
            "sort": "none"
        })
    );
}

#[test]
fn config_path_does_not_load_invalid_configuration() {
    let fixture = Fixture::new();
    fixture.write_config(b"{");

    let output = fixture.run(&["config", "path"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("path output must be valid JSON");

    let expected_path = fixture.directory.join(".mini-grep.json");

    assert_eq!(
        document,
        serde_json::json!({
            "encoding": "utf8",
            "value": expected_path
                .to_str()
                .expect("fixture path must be UTF-8")
        })
    );
}

#[test]
fn configuration_inspection_does_not_create_files() {
    let fixture = Fixture::new();

    for command in ["show", "path"] {
        let output = fixture.run(&["config", command]);

        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());

        serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .expect("inspection output must be valid JSON");

        assert!(!fixture.directory.join(".mini-grep.json").exists());
        assert_eq!(
            fs::read_dir(&fixture.directory)
                .expect("fixture directory must be readable")
                .count(),
            1
        );
    }
}

#[test]
fn config_show_reports_invalid_configuration_without_stdout() {
    let fixture = Fixture::new();
    fixture.write_config(b"{");

    let output = fixture.run(&["config", "show"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("diagnostic must be UTF-8");

    assert!(stderr.contains("cannot parse configuration"), "{stderr}");
    assert!(stderr.contains(".mini-grep.json"), "{stderr}");
}

#[test]
fn configuration_utility_help_bypasses_invalid_configuration() {
    let fixture = Fixture::new();
    fixture.write_config(b"{");

    for arguments in [
        vec!["config", "--help"],
        vec!["config", "show", "--help"],
        vec!["config", "path", "--help"],
    ] {
        let output = fixture.run(&arguments);

        assert_eq!(output.status.code(), Some(0));
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn separator_preserves_config_as_a_literal_search_pattern() {
    let fixture = Fixture::new();
    fs::write(fixture.directory.join("show"), b"config\n").expect("search fixture must be written");

    let separated = fixture.run(&["--", "config", "show"]);

    assert_eq!(separated.status.code(), Some(0));
    assert_eq!(separated.stdout, b"show:config\n");
    assert!(separated.stderr.is_empty());

    let explicit = fixture.run(&["-F", "-h", "config", "show"]);

    assert_eq!(explicit.status.code(), Some(0));
    assert_eq!(explicit.stdout, b"config\n");
    assert!(explicit.stderr.is_empty());
}

#[test]
fn completion_generation_bypasses_invalid_local_configuration() {
    let fixture = Fixture::new();
    fixture.write_config(b"{");

    for shell in ["bash", "zsh", "fish", "powershell"] {
        let output = fixture.run(&["completions", shell]);

        assert_eq!(output.status.code(), Some(0), "shell: {shell}");
        assert!(output.stderr.is_empty());

        let script = String::from_utf8(output.stdout).expect("completion script must be UTF-8");

        for expected in ["mini-grep", "json", "stats", "config", "completions"] {
            assert!(
                script.contains(expected),
                "shell {shell} omitted {expected}"
            );
        }

        assert_eq!(
            fs::read(fixture.directory.join(".mini-grep.json"))
                .expect("configuration must remain readable"),
            b"{"
        );
    }
}

#[test]
fn invalid_completion_commands_fail_before_loading_configuration() {
    let fixture = Fixture::new();
    fixture.write_config(b"{");

    for arguments in [
        vec!["completions"],
        vec!["completions", "unknown"],
        vec!["completions", "bash", "extra"],
    ] {
        let output = fixture.run(&arguments);

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8(output.stderr).expect("diagnostic must be UTF-8");

        assert!(!stderr.is_empty());
        assert!(!stderr.contains(".mini-grep.json"), "{stderr}");
    }
}

#[test]
fn completion_help_bypasses_invalid_local_configuration() {
    let fixture = Fixture::new();
    fixture.write_config(b"{");

    let output = fixture.run(&["completions", "--help"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(!output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn separator_preserves_completions_as_a_search_pattern() {
    let fixture = Fixture::new();
    fs::write(fixture.directory.join("sample.txt"), b"completions\n")
        .expect("search fixture must be written");

    let output = fixture.run(&["--", "completions", "sample.txt"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"sample.txt:completions\n");
    assert!(output.stderr.is_empty());
}
