use super::LineMatch;

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
