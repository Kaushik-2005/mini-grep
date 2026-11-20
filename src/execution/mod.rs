pub(crate) mod cancel;
pub(crate) mod ordering;
pub(crate) mod parallel;
pub(crate) mod sequential;

use self::cancel::CancellationToken;
use crate::input::binary::BinaryPolicy;
use crate::search::{SearchEvent, SearchLine};

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
pub(crate) struct SearchOptions<'a> {
    pub(crate) before_context: usize,
    pub(crate) after_context: usize,
    pub(crate) max_count: Option<usize>,
    pub(crate) quiet: bool,
    pub(crate) output_mode: OutputMode,
    pub(crate) cancellation: &'a CancellationToken,
    pub(crate) binary_policy: BinaryPolicy,
}

/// An execution result for the current input.
/// The caller supplies its path separately.
#[derive(Debug)]
pub(crate) enum ExecutionEvent {
    BeginFile,
    Path,
    Count(u128),
    Search(SearchEvent),
    OnlyMatching(SearchLine),
}
