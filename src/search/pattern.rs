use super::MatchSpan;
use super::regex::RegexMatcher;

#[derive(Debug, Clone, Copy)]
pub(crate) enum PatternMode {
    Regex,
    Literal,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CaseMode {
    Sensitive,
    Insensitive,
    Smart,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Selection {
    Matching,
    NonMatching,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum MatchScope {
    Substring,
    Word,
    Line,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PatternOptions {
    pub(crate) mode: PatternMode,
    pub(crate) case: CaseMode,
    pub(crate) selection: Selection,
    pub(crate) scope: MatchScope,
}

impl Default for PatternOptions {
    fn default() -> Self {
        Self {
            mode: PatternMode::Regex,
            case: CaseMode::Sensitive,
            selection: Selection::Matching,
            scope: MatchScope::Substring,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PatternMatcher {
    patterns: Vec<RegexMatcher>,
    selection: Selection,
}

impl PatternMatcher {
    pub(crate) fn new(
        patterns: &[String],
        options: PatternOptions,
    ) -> Result<Self, ::regex::Error> {
        let ignore_case = match options.case {
            CaseMode::Sensitive => false,
            CaseMode::Insensitive => true,
            CaseMode::Smart => !patterns
                .iter()
                .any(|pattern| pattern.chars().any(char::is_uppercase)),
        };

        let patterns = patterns
            .iter()
            .map(|pattern| {
                let expression = match options.mode {
                    PatternMode::Regex => pattern.to_owned(),
                    PatternMode::Literal => ::regex::escape(pattern),
                };

                let expression = with_scope(&expression, options.scope)?;

                RegexMatcher::new(&expression, ignore_case)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            patterns,
            selection: options.selection,
        })
    }

    pub(crate) fn is_selected(&self, text: &str) -> bool {
        let matched = self.patterns.iter().any(|pattern| pattern.is_match(text));

        match self.selection {
            Selection::Matching => matched,
            Selection::NonMatching => !matched,
        }
    }

    /// Returns actual matches independently of line selection.
    pub(crate) fn find_spans(&self, text: &str) -> Vec<MatchSpan> {
        let mut spans: Vec<_> = self
            .patterns
            .iter()
            .flat_map(|pattern| pattern.find_spans(text))
            .collect();

        spans.sort_unstable();
        spans.dedup();

        spans
    }
}

fn with_scope(expression: &str, scope: MatchScope) -> Result<String, ::regex::Error> {
    let ast = ::regex_syntax::ast::parse::Parser::new()
        .parse(expression)
        .map_err(|error| ::regex::Error::Syntax(error.to_string()))?;

    let expression = ast.to_string();

    Ok(match scope {
        MatchScope::Substring => expression,
        MatchScope::Word => {
            format!(r"\b{{start-half}}(?:{expression})\b{{end-half}}")
        }
        MatchScope::Line => format!(r"\A(?:{expression})\z"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(patterns: &[&str], options: PatternOptions) -> PatternMatcher {
        let patterns: Vec<String> = patterns
            .iter()
            .map(|pattern| (*pattern).to_owned())
            .collect();

        PatternMatcher::new(&patterns, options).expect("valid patterns must compile")
    }

    #[test]
    fn default_mode_interprets_regex_syntax() {
        let matcher = matcher(&["R.st"], PatternOptions::default());

        assert!(matcher.is_selected("Rust"));
        assert!(!matcher.is_selected("Rst"));
    }

    #[test]
    fn literal_mode_escapes_regex_syntax() {
        let options = PatternOptions {
            mode: PatternMode::Literal,
            ..PatternOptions::default()
        };

        for (pattern, text) in [("[", "a[b"), (".", "a.b"), ("(?i)Rust", "(?i)Rust")] {
            let matcher = matcher(&[pattern], options);

            assert!(matcher.is_selected(text));
            assert!(!matcher.is_selected("unrelated"));
        }
    }

    #[test]
    fn invalid_regex_patterns_return_errors() {
        let patterns = vec![String::from("[")];

        assert!(PatternMatcher::new(&patterns, PatternOptions::default()).is_err());
    }

    #[test]
    fn insensitive_matching_preserves_original_unicode_spans() {
        let matcher = matcher(
            &["σ"],
            PatternOptions {
                case: CaseMode::Insensitive,
                ..PatternOptions::default()
            },
        );

        let text = "é Σ";

        assert!(matcher.is_selected(text));
        assert_eq!(
            matcher.find_spans(text),
            vec![MatchSpan::new(text, 3, 5).expect("valid expected range")]
        );
    }

    #[test]
    fn smart_case_checks_all_pattern_text() {
        let options = PatternOptions {
            case: CaseMode::Smart,
            ..PatternOptions::default()
        };

        let lowercase = matcher(&["rust"], options);
        assert!(lowercase.is_selected("RUST"));

        let mixed = matcher(&["rust", "GO"], options);
        assert!(mixed.is_selected("rust"));
        assert!(mixed.is_selected("GO"));
        assert!(!mixed.is_selected("RUST"));
        assert!(!mixed.is_selected("go"));
    }

    #[test]
    fn inversion_applies_after_combining_patterns() {
        let matcher = matcher(
            &["Rust", "Go"],
            PatternOptions {
                selection: Selection::NonMatching,
                ..PatternOptions::default()
            },
        );

        assert!(!matcher.is_selected("Rust"));
        assert!(!matcher.is_selected("Go"));
        assert!(matcher.is_selected("Python"));
        assert!(matcher.is_selected(""));
        assert!(matcher.find_spans("Python").is_empty());
    }

    #[test]
    fn empty_pattern_set_matches_nothing_before_inversion() {
        let matching = matcher(&[], PatternOptions::default());
        assert!(!matching.is_selected("Rust"));
        assert!(!matching.is_selected(""));

        let inverted = matcher(
            &[],
            PatternOptions {
                selection: Selection::NonMatching,
                ..PatternOptions::default()
            },
        );

        assert!(inverted.is_selected("Rust"));
        assert!(inverted.is_selected(""));
        assert!(inverted.find_spans("Rust").is_empty());
    }

    #[test]
    fn merges_duplicate_spans_and_preserves_distinct_overlaps() {
        let matcher = matcher(&["ana", "nan", "ana"], PatternOptions::default());

        let text = "banana";

        assert_eq!(
            matcher.find_spans(text),
            vec![
                MatchSpan::new(text, 1, 4).expect("valid first range"),
                MatchSpan::new(text, 2, 5).expect("valid second range"),
            ]
        );
    }

    #[test]
    fn empty_pattern_preserves_zero_width_matches() {
        let matcher = matcher(&[""], PatternOptions::default());
        let text = "é";

        assert!(matcher.is_selected(""));
        assert!(matcher.is_selected(text));

        assert_eq!(
            matcher.find_spans(text),
            vec![
                MatchSpan::new(text, 0, 0).expect("valid start boundary"),
                MatchSpan::new(text, 2, 2).expect("valid end boundary"),
            ]
        );
    }

    #[test]
    fn explicit_inline_flags_can_override_default_case_settings() {
        let matcher = matcher(
            &["(?-i:Rust)"],
            PatternOptions {
                case: CaseMode::Insensitive,
                ..PatternOptions::default()
            },
        );

        assert!(matcher.is_selected("Rust"));
        assert!(!matcher.is_selected("rust"));
    }

    #[test]
    fn word_scope_checks_unicode_word_boundaries() {
        let matcher = matcher(
            &["cat"],
            PatternOptions {
                scope: MatchScope::Word,
                ..PatternOptions::default()
            },
        );

        assert!(matcher.is_selected("cat"));
        assert!(matcher.is_selected("(cat)"));

        for text in ["scatter", "_cat", "cat_", "écat", "caté", "cat\u{301}"] {
            assert!(!matcher.is_selected(text), "text: {text}");
        }
    }

    #[test]
    fn word_scope_accepts_punctuation_only_patterns() {
        let matcher = matcher(
            &["@"],
            PatternOptions {
                scope: MatchScope::Word,
                ..PatternOptions::default()
            },
        );

        assert!(matcher.is_selected("@"));
        assert!(matcher.is_selected(" @ "));
        assert!(!matcher.is_selected("x@y"));
        assert!(!matcher.is_selected("x@"));
        assert!(!matcher.is_selected("@x"));
    }

    #[test]
    fn word_scope_handles_alternatives_and_shared_separators() {
        let matcher = matcher(
            &["a|ab"],
            PatternOptions {
                scope: MatchScope::Word,
                ..PatternOptions::default()
            },
        );

        let text = "ab ab";

        assert_eq!(
            matcher.find_spans(text),
            vec![
                MatchSpan::new(text, 0, 2).expect("valid first range"),
                MatchSpan::new(text, 3, 5).expect("valid second range"),
            ]
        );
    }

    #[test]
    fn line_scope_handles_alternatives_and_rejects_surrounding_text() {
        let matcher = matcher(
            &["a|ab"],
            PatternOptions {
                scope: MatchScope::Line,
                ..PatternOptions::default()
            },
        );

        assert!(matcher.is_selected("ab"));
        assert!(!matcher.is_selected("zab"));
        assert!(!matcher.is_selected("abz"));

        assert_eq!(
            matcher.find_spans("ab"),
            vec![MatchSpan::new("ab", 0, 2).expect("valid whole-line range")]
        );
    }

    #[test]
    fn line_scope_preserves_literal_syntax_and_spaces() {
        let matcher = matcher(
            &["(?x) a# "],
            PatternOptions {
                mode: PatternMode::Literal,
                scope: MatchScope::Line,
                ..PatternOptions::default()
            },
        );

        assert!(matcher.is_selected("(?x) a# "));
        assert!(!matcher.is_selected("a#"));
        assert!(!matcher.is_selected("(?x) a#"));
    }

    #[test]
    fn scoped_patterns_preserve_verbose_mode_with_trailing_comments() {
        for scope in [MatchScope::Word, MatchScope::Line] {
            let matcher = matcher(
                &["(?x) Rust # trailing comment"],
                PatternOptions {
                    scope,
                    ..PatternOptions::default()
                },
            );

            assert!(matcher.is_selected("Rust"));
            assert!(!matcher.is_selected("RUST"));

            assert_eq!(
                matcher.find_spans("Rust"),
                vec![MatchSpan::new("Rust", 0, 4).expect("valid range")]
            );
        }
    }

    #[test]
    fn empty_patterns_obey_word_and_line_scope() {
        let word = matcher(
            &[""],
            PatternOptions {
                scope: MatchScope::Word,
                ..PatternOptions::default()
            },
        );

        assert!(word.is_selected(""));
        assert!(word.is_selected(" "));
        assert!(!word.is_selected("abc"));

        let line = matcher(
            &[""],
            PatternOptions {
                scope: MatchScope::Line,
                ..PatternOptions::default()
            },
        );

        assert!(line.is_selected(""));
        assert!(!line.is_selected(" "));
        assert!(!line.is_selected("abc"));
    }

    #[test]
    fn inline_flags_do_not_override_outer_scope_boundaries() {
        let word = matcher(
            &["(?-u:cat)"],
            PatternOptions {
                scope: MatchScope::Word,
                ..PatternOptions::default()
            },
        );

        assert!(word.is_selected("cat"));
        assert!(!word.is_selected("écat"));

        let line = matcher(
            &["(?m)^Rust$"],
            PatternOptions {
                scope: MatchScope::Line,
                ..PatternOptions::default()
            },
        );

        assert!(line.is_selected("Rust"));
        assert!(!line.is_selected("Rust\nother"));
    }
}
