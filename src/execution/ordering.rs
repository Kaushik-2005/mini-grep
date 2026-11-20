use std::path::PathBuf;

use super::cancel::CancellationToken;
use crate::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortOrder {
    None,
    Path,
}

/// Path ordering completes discovery and reports its errors before yielding paths.
pub(crate) fn prepare<'a>(
    mut candidates: impl Iterator<Item = Result<PathBuf, AppError>> + 'a,
    order: SortOrder,
    cancellation: &CancellationToken,
    report: &mut impl FnMut(AppError) -> Result<(), AppError>,
) -> Result<Box<dyn Iterator<Item = Result<PathBuf, AppError>> + 'a>, AppError> {
    cancellation.check()?;

    if order == SortOrder::None {
        return Ok(Box::new(candidates));
    }

    let mut paths = Vec::new();

    loop {
        cancellation.check()?;
        let candidate = candidates.next();
        cancellation.check()?;

        match candidate {
            Some(Ok(path)) => paths.push(path),
            Some(Err(error)) => report(error)?,
            None => break,
        }
    }

    cancellation.check()?;
    paths.sort();
    cancellation.check()?;

    Ok(Box::new(paths.into_iter().map(Ok)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io;

    #[test]
    fn unsorted_preparation_leaves_discovery_lazy() {
        let cancellation = CancellationToken::default();
        let pulls = Cell::new(0);

        let candidates = std::iter::once_with(|| {
            pulls.set(pulls.get() + 1);
            Ok(PathBuf::from("sample.txt"))
        });

        let mut report = |_| Ok(());
        let mut prepared = prepare(candidates, SortOrder::None, &cancellation, &mut report)
            .expect("unsorted preparation must succeed");

        assert_eq!(pulls.get(), 0);

        assert_eq!(
            prepared
                .next()
                .expect("candidate must exist")
                .expect("candidate must succeed"),
            PathBuf::from("sample.txt"),
        );
        assert_eq!(pulls.get(), 1);
    }

    #[test]
    fn sorted_preparation_observes_cancellation_during_discovery() {
        let cancellation = CancellationToken::default();

        let candidates = std::iter::once_with(|| {
            cancellation.cancel();
            Ok(PathBuf::from("sample.txt"))
        });

        let mut report = |_| Ok(());
        let result = prepare(candidates, SortOrder::Path, &cancellation, &mut report);

        assert!(matches!(result, Err(AppError::Cancelled)));
    }

    #[test]
    fn diagnostic_failure_stops_sorted_discovery() {
        let cancellation = CancellationToken::default();
        let pulls = Cell::new(0);

        let candidates = vec![
            Err(AppError::Usage("simulated discovery error")),
            Ok(PathBuf::from("later.txt")),
        ]
        .into_iter()
        .inspect(|_| pulls.set(pulls.get() + 1));

        let mut report = |_| {
            Err(AppError::Diagnostic(io::Error::other(
                "diagnostic output failed",
            )))
        };

        let result = prepare(candidates, SortOrder::Path, &cancellation, &mut report);

        assert!(matches!(result, Err(AppError::Diagnostic(_))));
        assert_eq!(pulls.get(), 1);
    }
}
