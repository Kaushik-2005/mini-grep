use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(contents: &[u8]) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must follow the Unix epoch")
            .as_nanos();

        let root = std::env::temp_dir().join(format!(
            "mini-grep-pipe-{}-{timestamp}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));

        fs::create_dir(&root).expect("fixture directory must be created");
        fs::write(root.join("target.txt"), contents).expect("fixture file must be written");

        Self { root }
    }

    fn run_with_closed_stdout(&self, arguments: &[&str], paths: &[u8]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
            .current_dir(&self.root)
            .args(arguments)
            .args(["--files-from", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("mini-grep must start");

        let stdout = child.stdout.take().expect("stdout must be piped");
        drop(stdout);

        let mut stdin = child.stdin.take().expect("stdin must be piped");
        stdin.write_all(paths).expect("path list must be supplied");
        drop(stdin);

        let deadline = Instant::now() + Duration::from_secs(15);

        while child
            .try_wait()
            .expect("child status must be available")
            .is_none()
        {
            if Instant::now() >= deadline {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .expect("timed-out child must be reaped");

                panic!(
                    "mini-grep did not stop after stdout closed; stderr: {}",
                    String::from_utf8_lossy(&output.stderr),
                );
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        child.wait_with_output().expect("mini-grep must finish")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn closed_stdout_stops_search_and_enumeration_without_a_diagnostic() {
    let fixture = Fixture::new(b"Rust\n");

    let cases: &[&[&str]] = &[
        &["-n", "Rust"],
        &["-c", "absent"],
        &["--files"],
        &["--files", "-0"],
    ];

    for arguments in cases {
        let output = fixture.run_with_closed_stdout(arguments, b"target.txt\n");

        assert_eq!(
            output.status.code(),
            Some(0),
            "arguments: {arguments:?}; stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(output.stdout.is_empty());
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn closed_stdout_preserves_an_earlier_target_error() {
    let fixture = Fixture::new(b"Rust\n");
    let cases: &[&[&str]] = &[&["-n", "Rust"], &["--files"]];

    for arguments in cases {
        let output = fixture.run_with_closed_stdout(arguments, b"missing-before.txt\ntarget.txt\n");

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing-before.txt"), "{stderr}");
        assert!(!stderr.contains("cannot write output"), "{stderr}");
        assert!(!stderr.contains("panicked"), "{stderr}");
    }
}

#[test]
fn broken_pipe_during_search_skips_later_targets() {
    let contents = b"Rust Rust\n".repeat(100_000);
    let fixture = Fixture::new(&contents);

    let cases: &[&[&str]] = &[
        &["-n", "Rust"],
        &["-o", "Rust"],
        &["--heading", "-n", "Rust"],
    ];

    for arguments in cases {
        let output = fixture.run_with_closed_stdout(arguments, b"target.txt\nmissing-after.txt\n");

        assert_eq!(
            output.status.code(),
            Some(0),
            "arguments: {arguments:?}; stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn broken_pipe_during_enumeration_skips_later_targets() {
    let fixture = Fixture::new(b"Rust\n");
    let mut paths = b"target.txt\n".repeat(10_000);
    paths.extend_from_slice(b"missing-after.txt\n");

    for arguments in [&["--files"][..], &["--files", "-0"][..]] {
        let output = fixture.run_with_closed_stdout(arguments, &paths);

        assert_eq!(
            output.status.code(),
            Some(0),
            "arguments: {arguments:?}; stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn parallel_workers_stop_when_stdout_closes() {
    let contents = b"Rust Rust\n".repeat(100_000);
    let fixture = Fixture::new(&contents);
    let paths = b"target.txt\n".repeat(32);

    let cases: &[&[&str]] = &[
        &["-j2", "-n", "Rust"],
        &["-j2", "-no", "Rust"],
        &["-j2", "--heading", "-n", "Rust"],
        &["-j2", "--color=always", "-n", "Rust"],
    ];

    for arguments in cases {
        let output = fixture.run_with_closed_stdout(arguments, &paths);

        assert_eq!(
            output.status.code(),
            Some(0),
            "arguments: {arguments:?}; stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(output.stdout.is_empty());
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn parallel_broken_pipe_preserves_an_earlier_discovery_error() {
    let contents = b"Rust\n".repeat(100_000);
    let fixture = Fixture::new(&contents);

    let mut paths = b"missing-before.txt\n".to_vec();
    paths.extend_from_slice(&b"target.txt\n".repeat(32));

    let output = fixture.run_with_closed_stdout(&["-j2", "-n", "Rust"], &paths);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing-before.txt"), "{stderr}");
    assert!(!stderr.contains("cannot write output"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}
