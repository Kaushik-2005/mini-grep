use ::regex::{Regex, RegexBuilder};

use super::MatchSpan;

#[derive(Debug)]
pub(super) struct RegexMatcher {
    compiled: Regex,
}

impl RegexMatcher {
    pub(super) fn new(pattern: &str, ignore_case: bool) -> Result<Self, ::regex::Error> {
        let compiled = RegexBuilder::new(pattern)
            .case_insensitive(ignore_case)
            .build()?;

        Ok(Self { compiled })
    }

    pub(super) fn is_match(&self, text: &str) -> bool {
        self.compiled.is_match(text)
    }

    pub(super) fn find_spans(&self, text: &str) -> Vec<MatchSpan> {
        self.compiled
            .find_iter(text)
            .map(|matched| {
                // Regex guarantees ordered, in-bounds UTF-8 boundaries
                // for matches in the same text passed to find_iter
                MatchSpan::new(text, matched.start(), matched.end())
                    .expect("regex matches must have valid UTF-8 byte ranges")
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interprets_regex_syntax() {
        let matcher = RegexMatcher::new(r"R.st", false).expect("valid regex must compile");

        assert!(matcher.is_match("Learn Rust"));
        assert!(!matcher.is_match("Learn Rst"));
    }

    #[test]
    fn invalid_and_unsupported_patterns_return_errors() {
        for pattern in ["[", "(?=Rust)", r"(Rust)\1"] {
            assert!(
                RegexMatcher::new(pattern, false).is_err(),
                "pattern must fail: {pattern}"
            );
        }
    }

    #[test]
    fn sensitive_matching_distinguishes_case() {
        let matcher = RegexMatcher::new("Rust", false).expect("valid regex must compile");

        assert!(matcher.is_match("Rust"));
        assert!(!matcher.is_match("rust"));
    }

    #[test]
    fn insensitive_matching_preserves_original_text_offsets() {
        let matcher = RegexMatcher::new("RUST", true).expect("valid regex must compile");

        let text = "é rUsT";
        let spans = matcher.find_spans(text);

        assert_eq!(
            spans,
            vec![MatchSpan::new(text, 3, 7).expect("valid expected range")]
        );
        assert_eq!(text.get(spans[0].start()..spans[0].end()), Some("rUsT"));
    }

    #[test]
    fn absent_patterns_return_no_spans() {
        let matcher = RegexMatcher::new("Rust", false).expect("valid regex must compile");

        assert!(!matcher.is_match("Go"));
        assert!(matcher.find_spans("Go").is_empty());
    }

    #[test]
    fn finds_repeated_nonoverlapping_matches() {
        let matcher = RegexMatcher::new("Rust", false).expect("valid regex must compile");

        let text = "Rust Rust";

        assert_eq!(
            matcher.find_spans(text),
            vec![
                MatchSpan::new(text, 0, 4).expect("valid first range"),
                MatchSpan::new(text, 5, 9).expect("valid second range"),
            ]
        );
    }

    #[test]
    fn zero_width_matches_respect_unicode_boundaries() {
        let matcher = RegexMatcher::new("", false).expect("empty regex must compile");

        let text = "é";

        assert_eq!(
            matcher.find_spans(text),
            vec![
                MatchSpan::new(text, 0, 0).expect("valid start boundary"),
                MatchSpan::new(text, 2, 2).expect("valid end boundary"),
            ]
        );
    }

    #[test]
    fn insensitive_matching_uses_unicode_simple_case_folding() {
        let matcher = RegexMatcher::new("Σ", true).expect("valid regex must compile");

        assert!(matcher.is_match("σ"));
        assert!(matcher.is_match("ς"));
    }
}
