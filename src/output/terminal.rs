use std::io::{self, Write};

use crate::search::LineMatch;

pub(crate) fn write_matches(writer: &mut impl Write, matches: &[LineMatch<'_>]) -> io::Result<()> {
    for matched_line in matches {
        writeln!(writer, "{}:{}", matched_line.line_number, matched_line.text)?;
    }

    writer.flush()
}
