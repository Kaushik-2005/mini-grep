#[derive(Debug, PartialEq, Eq)]
pub struct LineMatch<'a> {
    pub line_number: usize,
    pub text: &'a str,
}
