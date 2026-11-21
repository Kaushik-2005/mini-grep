use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const PATTERN: &str = "Rust";
const MATCHING_LINE: &str = "Rust makes ownership explicit.\n";
const OTHER_LINE: &str = "Small steps build understanding.\n";
const MATCH_INTERVAL: usize = 64;
const LINES_PER_CHUNK: usize = 8_192;
const FILE_COUNT: usize = 64;

#[derive(Clone, Copy)]
enum Tool {
    MiniGrep,
    Ripgrep,
    GnuGrep,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::MiniGrep => "mini-grep",
            Self::Ripgrep => "ripgrep",
            Self::GnuGrep => "grep",
        }
    }
}

struct Fixture {
    directory: PathBuf,
    total_bytes: u64,
}

impl Fixture {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_nanos();

        let directory = std::env::temp_dir().join(format!(
            "mini-grep-cli-bench-{}-{timestamp}",
            std::process::id(),
        ));

        fs::create_dir(&directory).expect("benchmark directory must be created");

        // Establish cleanup before creating files.
        let mut fixture = Self {
            directory,
            total_bytes: 0,
        };

        let mut chunk = String::new();

        for index in 0..LINES_PER_CHUNK {
            if index % MATCH_INTERVAL == 0 {
                chunk.push_str(MATCHING_LINE);
            } else {
                chunk.push_str(OTHER_LINE);
            }
        }

        fixture.total_bytes = (chunk.len() * FILE_COUNT) as u64;

        let single = File::create(fixture.directory.join("single.txt"))
            .expect("single-file fixture must be created");
        let mut single = BufWriter::new(single);

        for _ in 0..FILE_COUNT {
            single
                .write_all(chunk.as_bytes())
                .expect("single-file fixture must be written");
        }

        single.flush().expect("single-file fixture must be flushed");

        let many = fixture.directory.join("many");
        fs::create_dir(&many).expect("directory fixture must be created");

        for index in 0..FILE_COUNT {
            fs::write(many.join(format!("part-{index:03}.txt")), chunk.as_bytes())
                .expect("directory fixture file must be written");
        }

        fixture
    }

    fn expected_counts(&self, target: &str, matches_per_chunk: usize) -> BTreeMap<PathBuf, usize> {
        if target == "single.txt" {
            BTreeMap::from([(PathBuf::from("single.txt"), matches_per_chunk * FILE_COUNT)])
        } else {
            (0..FILE_COUNT)
                .map(|index| {
                    (
                        PathBuf::from("many").join(format!("part-{index:03}.txt")),
                        matches_per_chunk,
                    )
                })
                .collect()
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn search_command(
    tool: Tool,
    fixture: &Fixture,
    target: &str,
    threads: usize,
    pattern: &str,
    fixed_strings: bool,
) -> Command {
    let mut command = match tool {
        Tool::MiniGrep => Command::new(env!("CARGO_BIN_EXE_mini-grep")),
        Tool::Ripgrep => {
            let mut command = Command::new("rg");

            command.args(["--no-config", "--no-mmap", "--include-zero"]);

            command
        }
        Tool::GnuGrep => {
            assert_eq!(threads, 1, "GNU grep cases use one worker");

            let executable = std::env::var_os("GREP_BENCH_EXE").unwrap_or_else(|| "grep".into());
            let mut command = Command::new(executable);

            // This regex workload has ASCII-only input. Use an equivalent
            // POSIX ERE instead of relying on Rust-specific regex syntax.
            let grep_pattern = if fixed_strings {
                command.arg("-F");
                pattern
            } else {
                assert_eq!(pattern, r"^Rust\s+makes ownership explicit\.$");
                command.arg("-E");
                r"^Rust[[:space:]]+makes ownership explicit\.$"
            };

            command
                .current_dir(&fixture.directory)
                .stdin(Stdio::null())
                .env("LC_ALL", "C")
                .args(["-r", "-c", "-H", "-a", "--color=never"])
                .arg("--")
                .arg(grep_pattern)
                .arg(target);

            return command;
        }
    };

    command
        .current_dir(&fixture.directory)
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
        .arg(target);

    if fixed_strings {
        command.arg("-F");
    }

    command
}

fn verify_counts(
    tool: Tool,
    fixture: &Fixture,
    target: &str,
    threads: usize,
    pattern: &str,
    matches_per_chunk: usize,
    fixed_strings: bool,
) {
    let output = search_command(tool, fixture, target, threads, pattern, fixed_strings)
        .output()
        .expect("executable must run; check rg on PATH and GREP_BENCH_EXE or grep on PATH");

    let expected_exit = if matches_per_chunk == 0 { 1 } else { 0 };

    assert_eq!(
        output.status.code(),
        Some(expected_exit),
        "{} returned an unexpected exit status: {}",
        tool.name(),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        output.stderr.is_empty(),
        "{} wrote diagnostics: {}",
        tool.name(),
        String::from_utf8_lossy(&output.stderr),
    );

    // Relative fixture names contain only ASCII characters.
    let stdout = String::from_utf8(output.stdout).expect("fixture output must be UTF-8");
    let mut actual = BTreeMap::new();

    for line in stdout.lines() {
        let (path, count) = line
            .rsplit_once(':')
            .expect("count output must contain a path and count");
        let count = count
            .parse::<usize>()
            .expect("count must be an unsigned integer");

        assert!(
            actual.insert(PathBuf::from(path), count).is_none(),
            "duplicate count record: {path}",
        );
    }

    assert_eq!(
        actual,
        fixture.expected_counts(target, matches_per_chunk),
        "{} returned incorrect counts for {target} with {threads} workers",
        tool.name(),
    );
}

fn benchmark_repository(criterion: &mut Criterion) {
    let fixture = Fixture::new();

    for (workload, pattern, matches_per_chunk, fixed_strings) in [
        (
            "sparse_ascii",
            PATTERN,
            LINES_PER_CHUNK / MATCH_INTERVAL,
            true,
        ),
        ("absent_ascii", "__mini_grep_absent__", 0, true),
        ("dense_ascii", ".", LINES_PER_CHUNK, true),
        (
            "regex_sparse_ascii",
            r"^Rust\s+makes ownership explicit\.$",
            LINES_PER_CHUNK / MATCH_INTERVAL,
            false,
        ),
    ] {
        let mut group = criterion.benchmark_group(format!("cli/{workload}"));

        group.sample_size(20);
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(10));
        group.throughput(Throughput::Bytes(fixture.total_bytes));

        let expected_exit = if matches_per_chunk == 0 { 1 } else { 0 };

        for (target, worker_counts) in [
            ("single.txt", &[1_usize][..]),
            ("many", &[1_usize, 2, 4][..]),
        ] {
            for &threads in worker_counts {
                for tool in [Tool::MiniGrep, Tool::Ripgrep, Tool::GnuGrep] {
                    if matches!(tool, Tool::GnuGrep) && threads != 1 {
                        continue;
                    }

                    verify_counts(
                        tool,
                        &fixture,
                        target,
                        threads,
                        pattern,
                        matches_per_chunk,
                        fixed_strings,
                    );

                    let mut command =
                        search_command(tool, &fixture, target, threads, pattern, fixed_strings);

                    command.stdout(Stdio::null()).stderr(Stdio::null());

                    group.bench_function(
                        BenchmarkId::new(format!("{}/{target}", tool.name()), threads),
                        |bencher| {
                            bencher.iter(|| {
                                let status =
                                    command.status().expect("benchmark executable must run");

                                assert_eq!(
                                    status.code(),
                                    Some(expected_exit),
                                    "{} returned an unexpected status: {status}",
                                    tool.name(),
                                );
                            });
                        },
                    );
                }
            }
        }

        group.finish();
    }
}

criterion_group!(benches, benchmark_repository);
criterion_main!(benches);
