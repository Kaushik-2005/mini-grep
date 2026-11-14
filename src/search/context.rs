use std::collections::VecDeque;

use super::result::{SearchEvent, SearchLine};

/// Selects context around lines already marked as selected.
///
/// Feed consecutive, one-based lines from a single file.
/// Create a new selector for each file.
#[derive(Debug)]
pub(crate) struct ContextSelector {
    before: usize,
    after: usize,
    pending: VecDeque<SearchLine>,
    remaining_after: usize,
    last_emitted: Option<usize>,
}

impl ContextSelector {
    pub(crate) fn new(before: usize, after: usize) -> Self {
        Self {
            before,
            after,
            pending: VecDeque::new(),
            remaining_after: 0,
            last_emitted: None,
        }
    }

    pub(crate) fn push(&mut self, line: SearchLine) -> Vec<SearchEvent> {
        let mut events = Vec::new();

        if line.selected {
            while let Some(previous) = self.pending.pop_front() {
                self.emit(previous, &mut events);
            }

            self.emit(line, &mut events);
            self.remaining_after = self.after;
        } else if self.remaining_after > 0 {
            self.emit(line, &mut events);
            self.remaining_after -= 1;
        } else if self.before > 0 {
            if self.pending.len() == self.before {
                self.pending.pop_front();
            }

            self.pending.push_back(line);
        }

        events
    }

    fn emit(&mut self, line: SearchLine, events: &mut Vec<SearchEvent>) {
        let context_enabled = self.before > 0 || self.after > 0;

        if context_enabled
            && let Some(previous) = self.last_emitted
            && line.line_number.saturating_sub(previous) > 1
        {
            events.push(SearchEvent::GroupBreak);
        }

        self.last_emitted = Some(line.line_number);
        events.push(SearchEvent::Line(line));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::MatchSpan;

    fn line(number: usize, selected: bool) -> SearchLine {
        SearchLine {
            line_number: number,
            text: format!("line {number}"),
            spans: Vec::new(),
            selected,
        }
    }

    fn collect(before: usize, after: usize, selections: &[bool]) -> Vec<SearchEvent> {
        let mut selector = ContextSelector::new(before, after);

        selections
            .iter()
            .enumerate()
            .flat_map(|(index, &selected)| selector.push(line(index + 1, selected)))
            .collect()
    }

    #[test]
    fn empty_input_and_no_selections_emit_nothing() {
        assert!(collect(2, 2, &[]).is_empty());
        assert!(collect(2, 2, &[false, false, false]).is_empty());
    }

    #[test]
    fn zero_context_emits_only_selected_lines_without_separators() {
        assert_eq!(
            collect(0, 0, &[true, false, true]),
            vec![
                SearchEvent::Line(line(1, true)),
                SearchEvent::Line(line(3, true)),
            ]
        );
    }

    #[test]
    fn before_context_retains_only_the_requested_preceding_lines() {
        assert_eq!(
            collect(2, 0, &[false, false, false, true]),
            vec![
                SearchEvent::Line(line(2, false)),
                SearchEvent::Line(line(3, false)),
                SearchEvent::Line(line(4, true)),
            ]
        );
    }

    #[test]
    fn after_context_stops_at_the_requested_count() {
        assert_eq!(
            collect(0, 2, &[true, false, false, false]),
            vec![
                SearchEvent::Line(line(1, true)),
                SearchEvent::Line(line(2, false)),
                SearchEvent::Line(line(3, false)),
            ]
        );
    }

    #[test]
    fn context_is_clipped_at_file_boundaries() {
        assert_eq!(
            collect(5, 5, &[true, false, true]),
            vec![
                SearchEvent::Line(line(1, true)),
                SearchEvent::Line(line(2, false)),
                SearchEvent::Line(line(3, true)),
            ]
        );
    }

    #[test]
    fn overlapping_windows_emit_each_line_once() {
        assert_eq!(
            collect(1, 1, &[false, true, false, true, false]),
            vec![
                SearchEvent::Line(line(1, false)),
                SearchEvent::Line(line(2, true)),
                SearchEvent::Line(line(3, false)),
                SearchEvent::Line(line(4, true)),
                SearchEvent::Line(line(5, false)),
            ]
        );
    }

    #[test]
    fn adjacent_windows_form_one_group() {
        assert_eq!(
            collect(1, 1, &[true, false, false, true]),
            vec![
                SearchEvent::Line(line(1, true)),
                SearchEvent::Line(line(2, false)),
                SearchEvent::Line(line(3, false)),
                SearchEvent::Line(line(4, true)),
            ]
        );
    }

    #[test]
    fn omitted_lines_separate_groups_without_edge_separators() {
        assert_eq!(
            collect(1, 1, &[true, false, false, false, true]),
            vec![
                SearchEvent::Line(line(1, true)),
                SearchEvent::Line(line(2, false)),
                SearchEvent::GroupBreak,
                SearchEvent::Line(line(4, false)),
                SearchEvent::Line(line(5, true)),
            ]
        );
    }

    #[test]
    fn a_selected_line_inside_context_restarts_after_context() {
        assert_eq!(
            collect(0, 2, &[true, true, false, false, false]),
            vec![
                SearchEvent::Line(line(1, true)),
                SearchEvent::Line(line(2, true)),
                SearchEvent::Line(line(3, false)),
                SearchEvent::Line(line(4, false)),
            ]
        );
    }

    #[test]
    fn preserves_owned_unicode_text_selection_and_byte_spans() {
        let mut selector = ContextSelector::new(1, 0);

        let preceding = SearchLine {
            line_number: 1,
            text: String::from("नमस्ते"),
            spans: Vec::new(),
            selected: false,
        };

        assert!(selector.push(preceding).is_empty());

        let selected = SearchLine {
            line_number: 2,
            text: String::from("é Rust"),
            spans: vec![MatchSpan::new("é Rust", 3, 7).expect("expected range covers Rust")],
            selected: true,
        };

        assert_eq!(
            selector.push(selected),
            vec![
                SearchEvent::Line(SearchLine {
                    line_number: 1,
                    text: String::from("नमस्ते"),
                    spans: Vec::new(),
                    selected: false,
                }),
                SearchEvent::Line(SearchLine {
                    line_number: 2,
                    text: String::from("é Rust"),
                    spans: vec![
                        MatchSpan::new("é Rust", 3, 7).expect("expected range covers Rust"),
                    ],
                    selected: true,
                }),
            ]
        );
    }
}
