pub(crate) mod color;
pub(crate) mod json;
pub(crate) mod stats;
pub(crate) mod terminal;

#[derive(Debug, Clone, Copy)]
pub(crate) struct TerminalOptions {
    pub(crate) line_numbers: bool,
    pub(crate) show_filename: bool,
    pub(crate) column: bool,
    pub(crate) null: bool,
    pub(crate) color: bool,
}

impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            line_numbers: false,
            show_filename: true,
            column: false,
            null: false,
            color: false,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    #[default]
    Terminal,
    Json,
}
