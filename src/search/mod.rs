pub(crate) mod context;
mod literal;
mod pattern;
mod regex;
mod result;

pub use literal::search;
pub(crate) use pattern::{
    CaseMode, MatchScope, PatternMatcher, PatternMode, PatternOptions, Selection,
};
pub use result::{LineMatch, MatchSpan};
pub(crate) use result::{SearchEvent, SearchLine};
