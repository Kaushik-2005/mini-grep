use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use mini_grep::search::search;

const PATTERN: &str = "Rust";
const MATCHING_LINE: &str = "Rust makes ownership explicit.\n";
const OTHER_LINE: &str = "Small steps build understanding.\n";
const MATCH_INTERVAL: usize = 64;

fn sparse_ascii(line_count: usize) -> String {
    let mut text = String::new();

    for index in 0..line_count {
        if index % MATCH_INTERVAL == 0 {
            text.push_str(MATCHING_LINE);
        } else {
            text.push_str(OTHER_LINE);
        }
    }

    text
}

fn benchmark_sparse_ascii(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("literal/sparse_ascii");

    group.sample_size(50);
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(3));

    for line_count in [1_024_usize, 65_536] {
        let text = sparse_ascii(line_count);

        // Verify the workload before timing it.
        {
            let results = search(PATTERN, &text);

            assert_eq!(results.len(), line_count.div_ceil(MATCH_INTERVAL),);

            for (index, result) in results.iter().enumerate() {
                assert_eq!(result.line_number, index * MATCH_INTERVAL + 1,);
                assert_eq!(result.text, MATCHING_LINE.trim_end_matches('\n'),);
                assert_eq!(result.spans.len(), 1);
                assert_eq!(result.spans[0].start(), 0);
                assert_eq!(result.spans[0].end(), PATTERN.len());
            }
        }

        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(line_count),
            &text,
            |bencher, text| {
                bencher.iter(|| black_box(search(black_box(PATTERN), black_box(text.as_str()))));
            },
        );
    }

    group.finish();
}

fn benchmark_sparse_unicode(criterion: &mut Criterion) {
    const UNICODE_PATTERN: &str = "नमस्ते";
    const PREFIX: &str = "भाषा: ";
    const UNICODE_MATCHING_LINE: &str = "भाषा: नमस्ते दुनिया।\n";
    const UNICODE_OTHER_LINE: &str = "छोटे कदम सीखने में मदद करते हैं।\n";

    let mut group = criterion.benchmark_group("literal/sparse_unicode");

    group.sample_size(50);
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(3));

    for line_count in [1_024_usize, 65_536] {
        let mut text = String::new();

        for index in 0..line_count {
            if index % MATCH_INTERVAL == 0 {
                text.push_str(UNICODE_MATCHING_LINE);
            } else {
                text.push_str(UNICODE_OTHER_LINE);
            }
        }

        // Validate matching lines and byte spans outside timing.
        {
            let results = search(UNICODE_PATTERN, &text);

            assert_eq!(results.len(), line_count.div_ceil(MATCH_INTERVAL));

            for (index, result) in results.iter().enumerate() {
                assert_eq!(result.line_number, index * MATCH_INTERVAL + 1);
                assert_eq!(result.text, UNICODE_MATCHING_LINE.trim_end_matches('\n'));
                assert_eq!(result.spans.len(), 1);
                assert_eq!(result.spans[0].start(), PREFIX.len());
                assert_eq!(result.spans[0].end(), PREFIX.len() + UNICODE_PATTERN.len(),);
                assert_eq!(
                    &result.text[result.spans[0].start()..result.spans[0].end()],
                    UNICODE_PATTERN,
                );
            }
        }

        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(line_count),
            &text,
            |bencher, text| {
                bencher.iter(|| {
                    black_box(search(black_box(UNICODE_PATTERN), black_box(text.as_str())))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_sparse_ascii, benchmark_sparse_unicode);
criterion_main!(benches);
