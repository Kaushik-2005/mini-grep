use mini_grep::search::{LineMatch, search};

#[test]
fn returns_matching_lines_with_one_based_numbers() {
    let text = "first\nRust is useful\nthird\nRust is fast";

    assert_eq!(
        search("Rust", text),
        vec![
            LineMatch {
                line_number: 2,
                text: "Rust is useful",
            },
            LineMatch {
                line_number: 4,
                text: "Rust is fast",
            },
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
            LineMatch {
                line_number: 1,
                text: "first",
            },
            LineMatch {
                line_number: 2,
                text: "",
            },
            LineMatch {
                line_number: 3,
                text: "last",
            },
        ]
    );
}

#[test]
fn repeated_occurrences_produce_one_result_per_line() {
    assert_eq!(
        search("Rust", "Rust Rust Rust"),
        vec![LineMatch {
            line_number: 1,
            text: "Rust Rust Rust",
        }]
    );
}

#[test]
fn matches_unicode_text() {
    assert_eq!(
        search("नमस्ते", "hello\nनमस्ते दुनिया"),
        vec![LineMatch {
            line_number: 2,
            text: "नमस्ते दुनिया",
        }]
    );
}

#[test]
fn removes_crlf_line_endings() {
    assert_eq!(
        search("Rust", "Rust\r\nmore Rust\r\n"),
        vec![
            LineMatch {
                line_number: 1,
                text: "Rust",
            },
            LineMatch {
                line_number: 2,
                text: "more Rust",
            },
        ]
    );
}
