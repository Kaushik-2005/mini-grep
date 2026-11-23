use std::io::{self, Write};
use std::time::Duration;

use crate::execution::stats::SearchStats;

/// Writes as search summary without changing result output or exit status.
pub(crate) fn write_summary(
    writer: &mut impl Write,
    stats: &SearchStats,
    elapsed: Duration,
) -> io::Result<()> {
    writeln!(writer, "inputs_started: {}", stats.inputs_started)?;
    writeln!(
        writer,
        "inputs_with_selected_lines: {}",
        stats.inputs_with_selected_lines
    )?;
    writeln!(writer, "selected_lines: {}", stats.selected_lines)?;
    writeln!(writer, "matches: {}", stats.matches)?;
    writeln!(writer, "errors: {}", stats.errors)?;
    writeln!(writer, "elapsed_seconds: {:.6}", elapsed.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_counters_and_elapsed_seconds() {
        let stats = SearchStats {
            inputs_started: 3,
            inputs_with_selected_lines: 2,
            selected_lines: 4,
            matches: 7,
            errors: 1,
        };

        let mut output = Vec::new();
        write_summary(&mut output, &stats, Duration::from_millis(1250))
            .expect("in-memory output must succeed");

        assert_eq!(
            output,
            b"inputs_started: 3\n\
inputs_with_selected_lines: 2\n\
selected_lines: 4\n\
matches: 7\n\
errors: 1\n\
elapsed_seconds: 1.250000\n"
        );
    }

    #[test]
    fn writes_zero_measurements() {
        let mut output = Vec::new();

        write_summary(&mut output, &SearchStats::default(), Duration::ZERO)
            .expect("in-memory output must succeed");

        assert_eq!(
            output,
            b"inputs_started: 0\n\
inputs_with_selected_lines: 0\n\
selected_lines: 0\n\
matches: 0\n\
errors: 0\n\
elapsed_seconds: 0.000000\n"
        );
    }

    #[test]
    fn preserves_summary_write_errors() {
        struct FailingWriter;

        impl Write for FailingWriter {
            fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "summary pipe closed",
                ))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let error = write_summary(&mut FailingWriter, &SearchStats::default(), Duration::ZERO)
            .expect_err("summary write failure must propagate");

        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(error.to_string(), "summary pipe closed");
    }
}
