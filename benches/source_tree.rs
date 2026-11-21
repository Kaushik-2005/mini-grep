use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const PATTERN: &str = "Rust";

struct Snapshot {
    root: PathBuf,
    expected: BTreeMap<PathBuf, usize>,
    bytes: u64,
}

impl Snapshot {
    fn new(pattern: &str, fixed_strings: bool) -> Self {
        let expression = if fixed_strings {
            regex::escape(pattern)
        } else {
            pattern.to_owned()
        };
        let reference = regex::Regex::new(&expression).expect("benchmark pattern must compile");

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();

        let root = std::env::temp_dir().join(format!(
            "mini-grep-source-bench-{}-{timestamp}",
            std::process::id(),
        ));

        fs::create_dir(&root).expect("snapshot directory must be created");

        let mut snapshot = Self {
            root,
            expected: BTreeMap::new(),
            bytes: 0,
        };

        for directory in ["src", "tests", "benches"] {
            snapshot.copy_tree(Path::new(directory), &reference);
        }

        assert!(!snapshot.expected.is_empty(), "snapshot must contain files");

        snapshot
    }

    fn copy_tree(&mut self, relative: &Path, reference: &regex::Regex) {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let mut entries = fs::read_dir(source)
            .expect("source directory must be readable")
            .collect::<Result<Vec<_>, _>>()
            .expect("source entries must be readable");

        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let kind = entry.file_type().expect("file type must be readable");
            let relative = relative.join(entry.file_name());

            // Do not follow links while constructing the source snapshot.
            if kind.is_symlink() {
                continue;
            }

            if kind.is_dir() {
                self.copy_tree(&relative, reference);
                continue;
            }

            if !kind.is_file()
                || relative
                    .extension()
                    .is_none_or(|extension| extension != "rs")
            {
                continue;
            }

            let text =
                fs::read_to_string(entry.path()).expect("Rust source must be readable UTF-8");
            let count = text.lines().filter(|line| reference.is_match(line)).count();

            let output_path = Path::new("snapshot").join(&relative);
            let destination = self.root.join(&output_path);

            fs::create_dir_all(
                destination
                    .parent()
                    .expect("snapshot file must have a parent"),
            )
            .expect("snapshot parent must be created");
            fs::write(destination, text.as_bytes()).expect("snapshot file must be written");

            self.bytes += text.len() as u64;

            assert!(
                self.expected.insert(output_path, count).is_none(),
                "snapshot paths must be unique",
            );
        }
    }

    fn expected_exit(&self) -> i32 {
        if self.expected.values().any(|&count| count > 0) {
            0
        } else {
            1
        }
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn search_command(
    tool: &str,
    snapshot: &Snapshot,
    threads: usize,
    pattern: &str,
    fixed_strings: bool,
) -> Command {
    if tool == "grep" {
        assert!(
            fixed_strings && threads == 1,
            "GNU grep cases require literal mode and one worker",
        );

        let executable = std::env::var_os("GREP_BENCH_EXE").unwrap_or_else(|| "grep".into());
        let mut command = Command::new(executable);

        command
            .current_dir(&snapshot.root)
            .stdin(Stdio::null())
            .env("LC_ALL", "C")
            .args(["-r", "-F", "-c", "-H", "-a", "--color=never"])
            .arg("--")
            .arg(pattern)
            .arg("snapshot");

        return command;
    }

    let mut command = if tool == "mini-grep" {
        Command::new(env!("CARGO_BIN_EXE_mini-grep"))
    } else {
        let mut command = Command::new("rg");
        command.args(["--no-config", "--no-mmap", "--include-zero"]);
        command
    };

    command
        .current_dir(&snapshot.root)
        .stdin(Stdio::null())
        .args([
            "-c",
            "-H",
            "--color=never",
            "--sort=none",
            "--no-ignore",
            "--hidden",
            "--text",
        ])
        .arg(format!("-j{threads}"))
        .arg(pattern)
        .arg("snapshot");

    if fixed_strings {
        command.arg("-F");
    }

    command
}

fn verify_counts(
    tool: &str,
    snapshot: &Snapshot,
    threads: usize,
    pattern: &str,
    fixed_strings: bool,
) {
    let output = search_command(tool, snapshot, threads, pattern, fixed_strings)
        .output()
        .expect("executable must run; check rg on PATH and GREP_BENCH_EXE or grep on PATH");

    assert_eq!(
        output.status.code(),
        Some(snapshot.expected_exit()),
        "{tool} returned an unexpected status: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        output.stderr.is_empty(),
        "{tool} wrote diagnostics: {}",
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8(output.stdout).expect("snapshot paths must be UTF-8");
    let mut actual = BTreeMap::new();

    for line in stdout.lines() {
        let (path, count) = line
            .rsplit_once(':')
            .expect("count record must contain a path and count");
        let count = count
            .parse::<usize>()
            .expect("count must be an unsigned integer");

        assert!(
            actual.insert(PathBuf::from(path), count).is_none(),
            "duplicate output path: {path}",
        );
    }

    assert_eq!(
        actual, snapshot.expected,
        "{tool} returned incorrect counts with {threads} workers",
    );
}

fn benchmark_source_tree(criterion: &mut Criterion) {
    for (workload, pattern, fixed_strings) in [
        ("cli/source_tree", PATTERN, true),
        ("cli/source_tree_regex", r"\bRust\b", false),
    ] {
        let snapshot = Snapshot::new(pattern, fixed_strings);
        let expected_exit = snapshot.expected_exit();

        eprintln!(
            "{workload} snapshot: {} files, {} bytes",
            snapshot.expected.len(),
            snapshot.bytes,
        );

        let mut group = criterion.benchmark_group(workload);
        group.sample_size(20);
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(10));
        group.throughput(Throughput::Bytes(snapshot.bytes));

        for threads in [1, 2, 4] {
            for tool in ["mini-grep", "ripgrep", "grep"] {
                if tool == "grep" && (!fixed_strings || threads != 1) {
                    continue;
                }

                verify_counts(tool, &snapshot, threads, pattern, fixed_strings);

                let mut command = search_command(tool, &snapshot, threads, pattern, fixed_strings);
                command.stdout(Stdio::null()).stderr(Stdio::null());

                group.bench_function(BenchmarkId::new(tool, threads), |bencher| {
                    bencher.iter(|| {
                        let status = command.status().expect("executable must run");

                        assert_eq!(
                            status.code(),
                            Some(expected_exit),
                            "{tool} returned an unexpected status: {status}",
                        );
                    });
                });
            }
        }

        group.finish();
    }
}

criterion_group!(benches, benchmark_source_tree);
criterion_main!(benches);
