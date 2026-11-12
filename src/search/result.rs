#[derive(Debug, PartialEq, Eq)]
pub struct LineMatch<'a> {
    pub line_number: usize,
    pub text: &'a str,
    /// Sorted, unique byte spans into text; empty for inverted selections.
    pub spans: Vec<MatchSpan>,
}

/// A start-inclusive, end-exclusive byte range in UTF-8 text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MatchSpan {
    start: usize,
    end: usize,
}

impl MatchSpan {
    /// Returns None for reversed, out-of-bounds, or invalid UTF-8 boundaries
    pub fn new(text: &str, start: usize, end: usize) -> Option<Self> {
        text.get(start..end).map(|_| Self { start, end })
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_byte_offsets_into_original_unicode_text() {
        let text = "é Rust";
        let span = MatchSpan::new(text, 3, 7).expect("the range covers Rust");

        assert_eq!(span.start(), 3);
        assert_eq!(span.end(), 7);
        assert!(!span.is_empty());
        assert_eq!(text.get(span.start()..span.end()), Some("Rust"));
    }

    #[test]
    fn rejects_reversed_ranges() {
        assert!(MatchSpan::new("Rust", 3, 2).is_none());
    }

    #[test]
    fn rejects_out_of_bounds_ranges() {
        assert!(MatchSpan::new("Rust", 0, 5).is_none());
        assert!(MatchSpan::new("Rust", 5, 5).is_none());
    }

    #[test]
    fn rejects_boundaries_inside_a_unicode_character() {
        let text = "é Rust";

        assert!(MatchSpan::new(text, 1, 2).is_none());
        assert!(MatchSpan::new(text, 0, 1).is_none());
    }

    #[test]
    fn accepts_zero_width_ranges_at_valid_boundaries() {
        let text = "é Rust";

        for offset in [0, 2, 3, 7] {
            let span =
                MatchSpan::new(text, offset, offset).expect("the offset is a character boundary");

            assert!(span.is_empty());
            assert_eq!(text.get(span.start()..span.end()), Some(""));
        }

        let empty = MatchSpan::new("", 0, 0).expect("empty text has a valid boundary at zero");

        assert!(empty.is_empty());
    }
}
