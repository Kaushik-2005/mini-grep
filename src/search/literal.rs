use super::LineMatch;

#[derive(Debug, Clone, Copy)]
pub(crate) enum CaseSensitivity {
    Sensitive,
    Insensitive,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Selection {
    Matching,
    NonMatching,
}

#[derive(Debug)]
pub(crate) struct LiteralMatcher {
    pattern: String,
    case: CaseSensitivity,
    selection: Selection,
}

impl LiteralMatcher {
    pub(crate) fn new(pattern: &str, case: CaseSensitivity, selection: Selection) -> Self {
        let pattern = match case {
            CaseSensitivity::Sensitive => pattern.to_owned(),
            CaseSensitivity::Insensitive => pattern.to_lowercase(),
        };

        Self {
            pattern,
            case,
            selection,
        }
    }

    pub(crate) fn is_selected(&self, line: &str) -> bool {
        let matched = match self.case {
            CaseSensitivity::Sensitive => line.contains(self.pattern.as_str()),
            CaseSensitivity::Insensitive => line.to_lowercase().contains(self.pattern.as_str()),
        };

        match self.selection {
            Selection::Matching => matched,
            Selection::NonMatching => !matched,
        }
    }
}

pub(crate) fn matches_line(pattern: &str, line: &str) -> bool {
    line.contains(pattern)
}

pub fn search<'a>(pattern: &str, text: &'a str) -> Vec<LineMatch<'a>> {
    let mut matches = Vec::new();

    for (index, line) in text.lines().enumerate() {
        if matches_line(pattern, line) {
            matches.push(LineMatch {
                line_number: index + 1,
                text: line,
            });
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_matching_preserves_case_distinctions() {
        let matcher = LiteralMatcher::new("Rust", CaseSensitivity::Sensitive, Selection::Matching);

        assert!(matcher.is_selected("Learn Rust"));
        assert!(!matcher.is_selected("Learn rust"));
    }

    #[test]
    fn insensitive_matching_handles_ascii_case() {
        let matcher =
            LiteralMatcher::new("RUST", CaseSensitivity::Insensitive, Selection::Matching);

        assert!(matcher.is_selected("Learn rust"));
        assert!(matcher.is_selected("Learn RuSt"));
        assert!(!matcher.is_selected("Learn Go"));
    }

    #[test]
    fn insensitive_matching_handles_unicode_lowercasing() {
        let matcher =
            LiteralMatcher::new("CAFÉ", CaseSensitivity::Insensitive, Selection::Matching);

        assert!(matcher.is_selected("Un café"));
    }

    #[test]
    fn inversion_selects_only_nonmatching_lines() {
        let matcher =
            LiteralMatcher::new("Rust", CaseSensitivity::Sensitive, Selection::NonMatching);

        assert!(!matcher.is_selected("Learn Rust"));
        assert!(matcher.is_selected("Learn Go"));
        assert!(matcher.is_selected(""));
    }

    #[test]
    fn inversion_happens_after_case_insensitive_matching() {
        let matcher =
            LiteralMatcher::new("RUST", CaseSensitivity::Insensitive, Selection::NonMatching);

        assert!(!matcher.is_selected("Learn rust"));
        assert!(matcher.is_selected("Learn Go"));
    }

    #[test]
    fn empty_pattern_selects_every_line() {
        for case in [CaseSensitivity::Sensitive, CaseSensitivity::Insensitive] {
            let matcher = LiteralMatcher::new("", case, Selection::Matching);

            assert!(matcher.is_selected(""));
            assert!(matcher.is_selected("Rust"));
        }
    }

    #[test]
    fn inverted_empty_pattern_selects_no_lines() {
        for case in [CaseSensitivity::Sensitive, CaseSensitivity::Insensitive] {
            let matcher = LiteralMatcher::new("", case, Selection::NonMatching);

            assert!(!matcher.is_selected(""));
            assert!(!matcher.is_selected("Rust"));
        }
    }

    #[test]
    fn lowercasing_does_not_apply_full_case_folding_or_normalization() {
        let folding_example =
            LiteralMatcher::new("STRASSE", CaseSensitivity::Insensitive, Selection::Matching);

        assert!(!folding_example.is_selected("Straße"));

        let normalization_example =
            LiteralMatcher::new("É", CaseSensitivity::Insensitive, Selection::Matching);

        assert!(!normalization_example.is_selected("e\u{301}"));
    }
}
