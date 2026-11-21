use std::hint::black_box;
use std::io::{self, BufRead, BufReader, Cursor};
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const PATTERN: &[u8] = b"Rust";
const BUFFER_CAPACITY: usize = 64 * 1024;
const LINE_COUNT: usize = 524_288;
const MATCH_INTERVAL: usize = 64;

#[derive(Clone, Copy)]
enum Algorithm {
    WholeLine,
    Chunked,
}

impl Algorithm {
    fn name(self) -> &'static str {
        match self {
            Self::WholeLine => "whole_line",
            Self::Chunked => "chunked",
        }
    }

    fn count(self, reader: impl BufRead) -> io::Result<usize> {
        match self {
            Self::WholeLine => count_whole_lines(reader),
            Self::Chunked => count_chunks(reader),
        }
    }
}

fn count_whole_lines(mut reader: impl BufRead) -> io::Result<usize> {
    let mut line = Vec::new();
    let mut count = 0;

    loop {
        line.clear();

        if reader.read_until(b'\n', &mut line)? == 0 {
            return Ok(count);
        }

        if line.windows(PATTERN.len()).any(|bytes| bytes == PATTERN) {
            count += 1;
        }
    }
}

/// Counts lines containing exactly the fixed byte sequence `Rust`.
/// The prefix transition below is specific to this pattern.
fn count_chunks(mut reader: impl BufRead) -> io::Result<usize> {
    let mut count = 0;
    let mut prefix_length = 0;
    let mut line_matches = false;

    loop {
        let bytes = reader.fill_buf()?;

        if bytes.is_empty() {
            return Ok(count + usize::from(line_matches));
        }

        let consumed = bytes.len();

        for &byte in bytes {
            if byte == b'\n' {
                count += usize::from(line_matches);
                line_matches = false;
                prefix_length = 0;
                continue;
            }

            if line_matches {
                continue;
            }

            if byte == PATTERN[prefix_length] {
                prefix_length += 1;

                if prefix_length == PATTERN.len() {
                    line_matches = true;
                    prefix_length = 0;
                }
            } else {
                // `R` is the only nonempty prefix that can remain
                // after a mismatch for the fixed pattern `Rust`.
                prefix_length = usize::from(byte == b'R');
            }
        }

        reader.consume(consumed);
    }
}

fn verify_boundaries() {
    let cases: &[(&[u8], usize)] = &[
        (b"", 0),
        (b"\n\n", 0),
        (b"Rust", 1),
        (b"Rust\n", 1),
        (b"RustRust\nRust", 2),
        (b"Ru\nst", 0),
        (b"RRust\nRuRust\n", 2),
        (b"Rust\r\nrust\nRustRust\n", 2),
        (b"R\nRu\nRus\nRust\n", 1),
        (b"prefixRustSuffix\nlast", 1),
    ];

    for &capacity in &[1, 2, 3, 4, 7, BUFFER_CAPACITY] {
        for &(input, expected) in cases {
            for algorithm in [Algorithm::WholeLine, Algorithm::Chunked] {
                let reader = BufReader::with_capacity(capacity, Cursor::new(input));

                assert_eq!(
                    algorithm.count(reader).expect("memory input must read"),
                    expected,
                    "{} failed with capacity {capacity} and input {input:?}",
                    algorithm.name(),
                );
            }
        }
    }
}

fn make_short_lines() -> Vec<u8> {
    let mut text = String::new();

    for index in 0..LINE_COUNT {
        if index % MATCH_INTERVAL == 0 {
            text.push_str("Rust makes ownership explicit.\n");
        } else {
            text.push_str("Small steps build understanding.\n");
        }
    }

    text.into_bytes()
}

fn benchmark_reader(criterion: &mut Criterion) {
    verify_boundaries();

    let short_lines = make_short_lines();
    let mut long_line = vec![b'x'; short_lines.len()];
    long_line[..PATTERN.len()].copy_from_slice(PATTERN);
    *long_line.last_mut().expect("fixture must be nonempty") = b'\n';

    let mut long_line_absent = vec![b'x'; short_lines.len()];
    *long_line_absent
        .last_mut()
        .expect("fixture must be nonempty") = b'\n';

    let mut long_line_end = long_line_absent.clone();
    let match_end = long_line_end.len() - 1;
    let match_start = match_end - PATTERN.len();
    long_line_end[match_start..match_end].copy_from_slice(PATTERN);

    assert_eq!(short_lines.len(), 17_285_120);
    assert_eq!(long_line.len(), short_lines.len());

    let mut group = criterion.benchmark_group("reader/fixed_rust");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));

    for (workload, input, expected) in [
        (
            "short_lines",
            short_lines.as_slice(),
            LINE_COUNT / MATCH_INTERVAL,
        ),
        ("long_line", long_line.as_slice(), 1),
        ("long_line_absent", long_line_absent.as_slice(), 0),
        ("long_line_end", long_line_end.as_slice(), 1),
    ] {
        group.throughput(Throughput::Bytes(input.len() as u64));

        for algorithm in [Algorithm::WholeLine, Algorithm::Chunked] {
            let reader = BufReader::with_capacity(BUFFER_CAPACITY, Cursor::new(input));

            assert_eq!(
                algorithm.count(reader).expect("fixture must read"),
                expected,
                "{} returned an incorrect fixture count",
                algorithm.name(),
            );

            group.bench_function(BenchmarkId::new(workload, algorithm.name()), |bencher| {
                bencher.iter(|| {
                    let reader =
                        BufReader::with_capacity(BUFFER_CAPACITY, Cursor::new(black_box(input)));

                    black_box(algorithm.count(reader).expect("fixture must read"))
                });
            });
        }
    }

    group.finish();
}

criterion_group!(benches, benchmark_reader);
criterion_main!(benches);
