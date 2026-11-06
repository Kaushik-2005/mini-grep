use super::LineMatch;

pub fn search<'a>(pattern: &str, text: &'a str) -> Vec<LineMatch<'a>> {
    let mut matches = Vec::new();

    for (index, line) in text.lines().enumerate() {
        if line.contains(pattern) {
            matches.push(LineMatch {
                line_number: index + 1,
                text: line,
            });
        }
    }

    matches
}
