use mini_grep::search::search;
use proptest::prelude::*;

fn unicode_text(max_chars: usize) -> impl Strategy<Value = String> {
    let character = prop_oneof![
        6 => proptest::sample::select(vec![
            'a', 'b', 'R', 'u', 's', 't',
            '\u{e9}', '\u{928}', '\u{1f980}',
            '\n', '\r',
        ]),
        1 => any::<char>(),
    ];

    proptest::collection::vec(character, 0..=max_chars)
        .prop_map(|characters| characters.into_iter().collect())
}

fn search_case() -> impl Strategy<Value = (String, String)> {
    let independent = (unicode_text(8), unicode_text(128));

    let planted = (
        "[abRust\u{e9}\u{928}\u{1f980}]{1,8}",
        unicode_text(48),
        unicode_text(48),
    )
        .prop_map(|(pattern, prefix, suffix)| {
            let text = format!("{prefix}{pattern}{suffix}");
            (pattern, text)
        });

    prop_oneof![independent, planted]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn returns_exactly_the_lines_containing_the_literal_pattern(
        (pattern, text) in search_case(),
    ) {
        let results = search(&pattern, &text);

        let actual: Vec<_> = results
            .iter()
            .map(|line| (line.line_number, line.text))
            .collect();

        let expected: Vec<_> = text
            .lines()
            .enumerate()
            .filter(|(_, line)| line.contains(pattern.as_str()))
            .map(|(index, line)| (index + 1, line))
            .collect();

        prop_assert_eq!(actual, expected);
    }

    #[test]
    fn spans_are_valid_ordered_byte_ranges(
        (pattern, text) in search_case(),
    ) {
        for line in search(&pattern, &text) {
            prop_assert!(!line.spans.is_empty());

            for span in &line.spans {
                prop_assert_eq!(
                    line.text.get(span.start()..span.end()),
                    Some(pattern.as_str())
                );
                prop_assert_eq!(span.is_empty(), pattern.is_empty());
            }

            for pair in line.spans.windows(2) {
                prop_assert!(pair[0].start() < pair[1].start());
                prop_assert!(pair[0].end() <= pair[1].start());
            }
        }
    }

    #[test]
    fn separated_repetitions_report_every_occurrence(
        pattern in "[abRust\u{e9}\u{928}\u{1f980}]{1,8}",
        repetitions in 0usize..17,
    ) {
        let prefix = "λ|";
        let repeated = vec![pattern.as_str(); repetitions].join("|");
        let text = format!("{prefix}{repeated}");
        let results = search(&pattern, &text);

        if repetitions == 0 {
            prop_assert!(results.is_empty());
        } else {
            prop_assert_eq!(results.len(), 1);

            let line = &results[0];
            prop_assert_eq!(line.line_number, 1);
            prop_assert_eq!(line.text, text.as_str());
            prop_assert_eq!(line.spans.len(), repetitions);

            for (index, span) in line.spans.iter().enumerate() {
                let expected_start =
                    prefix.len() + index * (pattern.len() + 1);

                prop_assert_eq!(span.start(), expected_start);
                prop_assert_eq!(span.end(), expected_start + pattern.len());
            }
        }
    }

    #[test]
    fn empty_pattern_matches_each_character_boundary_once(
        text in unicode_text(128),
    ) {
        let results = search("", &text);
        let lines: Vec<_> = text.lines().collect();

        prop_assert_eq!(results.len(), lines.len());

        for (index, (result, original)) in
            results.iter().zip(lines.iter()).enumerate()
        {
            prop_assert_eq!(result.line_number, index + 1);
            prop_assert_eq!(result.text, *original);

            let boundaries: Vec<_> = original
                .char_indices()
                .map(|(offset, _)| offset)
                .chain(std::iter::once(original.len()))
                .collect();

            prop_assert_eq!(result.spans.len(), boundaries.len());

            for (span, boundary) in
                result.spans.iter().zip(boundaries)
            {
                prop_assert_eq!(span.start(), boundary);
                prop_assert_eq!(span.end(), boundary);
                prop_assert!(span.is_empty());
            }
        }
    }
}
