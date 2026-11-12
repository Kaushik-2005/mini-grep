use mini_grep::search::{LineMatch, MatchSpan, search};

fn expected<'a>(line_number: usize, text: &'a str, ranges: &[(usize, usize)]) -> LineMatch<'a> {
    LineMatch {
        line_number,
        text,
        spans: ranges
            .iter()
            .map(|&(start, end)| {
                MatchSpan::new(text, start, end).expect("expected ranges must be valid")
            })
            .collect(),
    }
}

#[test]
fn returns_matching_lines_with_one_based_numbers() {
    let text = "first\nRust is useful\nthird\nRust is fast";

    assert_eq!(
        search("Rust", text),
        vec![
            expected(2, "Rust is useful", &[(0, 4)]),
            expected(4, "Rust is fast", &[(0, 4)]),
        ]
    );
}

#[test]
fn returns_no_results_when_pattern_is_absent() {
    assert!(search("missing", "first\nsecond").is_empty());
}

#[test]
fn matching_is_case_sensitive() {
    assert!(search("rust", "Rust").is_empty());
}

#[test]
fn empty_input_returns_no_results() {
    assert!(search("Rust", "").is_empty());
    assert!(search("", "").is_empty());
}

#[test]
fn empty_pattern_matches_every_line() {
    assert_eq!(
        search("", "first\n\nlast\n"),
        vec![
            expected(
                1,
                "first",
                &[(0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5)],
            ),
            expected(2, "", &[(0, 0)]),
            expected(3, "last", &[(0, 0), (1, 1), (2, 2), (3, 3), (4, 4)],),
        ]
    );
}

#[test]
fn repeated_occurrences_produce_one_result_per_line() {
    assert_eq!(
        search("Rust", "Rust Rust Rust"),
        vec![expected(1, "Rust Rust Rust", &[(0, 4), (5, 9), (10, 14)],)]
    );
}

#[test]
fn matches_unicode_text() {
    assert_eq!(
        search("नमस्ते", "hello\nनमस्ते दुनिया"),
        vec![expected(2, "नमस्ते दुनिया", &[(0, "नमस्ते".len())],)]
    );
}

#[test]
fn removes_crlf_line_endings() {
    assert_eq!(
        search("Rust", "Rust\r\nmore Rust\r\n"),
        vec![
            expected(1, "Rust", &[(0, 4)]),
            expected(2, "more Rust", &[(5, 9)]),
        ]
    );
}

#[test]
fn offsets_after_a_multibyte_prefix_are_bytes() {
    assert_eq!(
        search("Rust", "é Rust"),
        vec![expected(1, "é Rust", &[(3, 7)])]
    );
}

#[test]
fn occurrences_within_one_literal_pattern_do_not_overlap() {
    assert_eq!(
        search("ana", "banana"),
        vec![expected(1, "banana", &[(1, 4)])]
    );
}
