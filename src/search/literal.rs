use super::{LineMatch, MatchSpan};

pub fn search<'a>(pattern: &str, text: &'a str) -> Vec<LineMatch<'a>> {
    let mut matches = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let spans: Vec<_> = line
            .match_indices(pattern)
            .map(|(start, matched)| {
                // str matches are valid UTF-8 slices of this same line
                MatchSpan::new(line, start, start + matched.len())
                    .expect("str matches must have valid UTF-8 byte ranges")
            })
            .collect();

        if !spans.is_empty() {
            matches.push(LineMatch {
                line_number: index + 1,
                text: line,
                spans,
            });
        }
    }

    matches
}
