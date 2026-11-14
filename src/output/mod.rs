pub(crate) mod terminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputMode {
    Lines,
    FilesWithMatches,
    FilesWithoutMatch,
    CountLines,
    CountMatches,
    OnlyMatching,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TerminalOptions {
    pub(crate) line_numbers: bool,
    pub(crate) show_filename: bool,
    pub(crate) column: bool,
    pub(crate) null: bool,
}

impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            line_numbers: false,
            show_filename: true,
            column: false,
            null: false,
        }
    }
}
