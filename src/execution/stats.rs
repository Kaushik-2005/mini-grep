/// Measurements collected during search execution.
///
/// Executors count each started input once and each input with selected
/// lines once. Context lines do not contribute to selection counters.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SearchStats {
    pub(crate) inputs_started: u128,
    pub(crate) inputs_with_selected_lines: u128,
    pub(crate) selected_lines: u128,
    pub(crate) matches: u128,
    pub(crate) errors: u128,
}

impl SearchStats {
    /// Combines measurements from independednt work.
    ///
    /// A snapshot must be merged only once. Counters saturate on overflow.
    pub(crate) fn merge(&mut self, other: Self) {
        self.inputs_started = self.inputs_started.saturating_add(other.inputs_started);

        self.inputs_with_selected_lines = self
            .inputs_with_selected_lines
            .saturating_add(other.inputs_with_selected_lines);

        self.selected_lines = self.selected_lines.saturating_add(other.selected_lines);

        self.matches = self.matches.saturating_add(other.matches);
        self.errors = self.errors.saturating_add(other.errors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_successful_and_partially_failed_inputs() {
        let successful = SearchStats {
            inputs_started: 1,
            inputs_with_selected_lines: 1,
            selected_lines: 2,
            matches: 3,
            errors: 0,
        };

        let partially_failed = SearchStats {
            inputs_started: 1,
            inputs_with_selected_lines: 1,
            selected_lines: 1,
            matches: 2,
            errors: 1,
        };

        let no_matches = SearchStats {
            inputs_started: 1,
            ..SearchStats::default()
        };

        let mut total = SearchStats::default();
        total.merge(successful);
        total.merge(partially_failed);
        total.merge(no_matches);

        assert_eq!(
            total,
            SearchStats {
                inputs_started: 3,
                inputs_with_selected_lines: 2,
                selected_lines: 3,
                matches: 5,
                errors: 1,
            }
        );
    }

    #[test]
    fn merging_empty_measurements_preserves_totals() {
        let original = SearchStats {
            inputs_started: 2,
            inputs_with_selected_lines: 1,
            selected_lines: 4,
            matches: 7,
            errors: 1,
        };

        let mut total = original;
        total.merge(SearchStats::default());

        assert_eq!(total, original);
    }

    #[test]
    fn counters_saturate_instead_of_overflowing() {
        let mut total = SearchStats {
            inputs_started: u128::MAX,
            inputs_with_selected_lines: u128::MAX,
            selected_lines: u128::MAX,
            matches: u128::MAX,
            errors: u128::MAX,
        };

        total.merge(SearchStats {
            inputs_started: 1,
            inputs_with_selected_lines: 1,
            selected_lines: 1,
            matches: 1,
            errors: 1,
        });

        assert_eq!(
            total,
            SearchStats {
                inputs_started: u128::MAX,
                inputs_with_selected_lines: u128::MAX,
                selected_lines: u128::MAX,
                matches: u128::MAX,
                errors: u128::MAX,
            }
        );
    }
}
