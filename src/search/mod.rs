mod literal;
mod result;

pub use literal::search;
pub(crate) use literal::{CaseSensitivity, LiteralMatcher, Selection};
pub use result::LineMatch;
