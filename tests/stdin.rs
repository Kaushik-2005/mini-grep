use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_with_stdin(arguments: &[&str], contents: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executable must start");

    {
        let mut input = child.stdin.take().expect("child stdin must be available");

        if let Err(error) = input.write_all(contents) {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe,
                "unexpected error writing child stdin: {error}"
            );
        }
    }

    child
        .wait_with_output()
        .expect("child output must be collected")
}

#[test]
fn explicit_stdin_searches_unicode_crlf_input() {
    let output = run_with_stdin(
        &["-n", "Rust", "-"],
        "first\r\né Rust\r\nRust Rust\r\n".as_bytes(),
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        "(standard input):2:é Rust\n\
        (standard input):3:Rust Rust\n"
            .as_bytes()
    );
}

#[test]
fn explicit_stdin_without_selections_returns_one() {
    for contents in [b"Go\n".as_slice(), b"".as_slice()] {
        let output = run_with_stdin(&["Rust", "-"], contents);

        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn stdin_reuses_output_modes_and_formatting() {
    let contents = "before\né Rust Rust\nafter\n".as_bytes();

    let cases: &[(&[&str], &str, i32, &str)] = &[
        (
            &["-n", "-C", "1"],
            "Rust",
            0,
            "(standard input)-1-before\n\
            (standard input):2:é Rust Rust\n\
            (standard input)-3-after\n",
        ),
        (
            &["--column", "-o"],
            "Rust",
            0,
            "(standard input):2:4:Rust\n\
            (standard input):2:9:Rust\n",
        ),
        (&["-c"], "Rust", 0, "(standard input):1\n"),
        (&["-h", "--count-matches"], "Rust", 0, "2\n"),
        (&["-l"], "Rust", 0, "(standard input)\n"),
        (&["--files-without-match"], "Go", 0, "(standard input)\n"),
        (&["--files-without-match"], "Rust", 1, ""),
        (
            &["--heading", "-n"],
            "Rust",
            0,
            "(standard input)\n2:é Rust Rust\n",
        ),
        (&["-0c"], "Rust", 0, "(standard input)\x001\n"),
        (&["-0l"], "Rust", 0, "(standard input)\0"),
        (&["-q"], "Rust", 0, ""),
    ];

    for &(flags, pattern, expected_code, expected) in cases {
        let mut arguments = flags.to_vec();
        arguments.extend_from_slice(&[pattern, "-"]);

        let output = run_with_stdin(&arguments, contents);

        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "flags: {flags:?}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes(), "flags: {flags:?}");
    }
}

#[test]
fn stdin_accepts_repeated_explicit_patterns() {
    let output = run_with_stdin(&["-n", "-e", "Rust", "-e", "Go", "-"], b"Rust Go\nPython\n");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"(standard input):1:Rust Go\n");
}

#[test]
fn stdin_limits_reuse_selection_and_trailing_context_rules() {
    let contents = b"Rust Rust\nafter\n\xff\n";

    let cases: &[(&[&str], &str)] = &[
        (
            &["-A", "1"],
            "(standard input):1:Rust Rust\n\
            (standard input)-2-after\n",
        ),
        (
            &["-o", "-A", "5"],
            "(standard input):1:Rust\n\
            (standard input):1:Rust\n",
        ),
        (&["-c", "-A", "5"], "(standard input):1\n"),
        (&["--count-matches", "-A", "5"], "(standard input):2\n"),
        (&["-q", "-A", "5"], ""),
    ];

    for limit_flag in ["-m", "--max-results"] {
        for &(flags, expected) in cases {
            let mut arguments = vec!["-n", limit_flag, "1"];
            arguments.extend_from_slice(flags);
            arguments.extend_from_slice(&["Rust", "-"]);

            let output = run_with_stdin(&arguments, contents);

            assert_eq!(
                output.status.code(),
                Some(0),
                "flags: {arguments:?}, stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            assert_eq!(output.stdout, expected.as_bytes());
        }

        let output = run_with_stdin(&[limit_flag, "0", "Rust", "-"], b"");

        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn stdin_read_errors_preserve_output_and_identify_the_source() {
    let inputs: &[(&[u8], bool)] = &[(b"Rust Rust\n\xff\n", true), (b"\xff\nRust\n", false)];

    let modes: &[(&[&str], &str)] = &[
        (&["-n"], "(standard input):1:Rust Rust\n"),
        (
            &["-no"],
            "(standard input):1:Rust\n\
            (standard input):1:Rust\n",
        ),
        (&["-c"], ""),
        (&["--count-matches"], ""),
    ];

    for &(contents, printed_selection) in inputs {
        for &(flags, partial_output) in modes {
            let mut arguments = flags.to_vec();
            arguments.extend_from_slice(&["Rust", "-"]);

            let output = run_with_stdin(&arguments, contents);

            assert_eq!(output.status.code(), Some(2));

            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.starts_with("mini-grep: "), "{stderr}");
            assert!(stderr.contains("cannot read"), "{stderr}");
            assert!(stderr.contains("(standard input)"), "{stderr}");
            assert!(!stderr.contains("panicked at"), "{stderr}");

            let expected = if printed_selection {
                partial_output
            } else {
                ""
            };

            assert_eq!(output.stdout, expected.as_bytes());
        }
    }
}

#[test]
fn omitted_target_automatically_searches_redirected_stdin() {
    let cases: &[&[&str]] = &[
        &["-n", "Rust"],
        &["-n", "-e", "Rust"],
        &["-n", "-e", "Rust", "-e", "Go"],
    ];

    for arguments in cases {
        let output = run_with_stdin(arguments, b"Rust Go\nPython\n");

        assert_eq!(
            output.status.code(),
            Some(0),
            "arguments: {arguments:?}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, b"(standard input):1:Rust Go\n");
    }
}

#[test]
fn explicit_file_target_overrides_redirected_stdin() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("text")
        .join("demo.txt");

    let output = run_with_stdin(
        &[
            "-n",
            "Rust",
            path.to_str().expect("fixture path must be valid Unicode"),
        ],
        b"Rust from redirected stdin\n",
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let expected = format!(
        "{}:2:Rust makes ownership explicit.\n\
        {}:4:Rust rewards careful design.\n",
        path.display(),
        path.display(),
    );

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn empty_redirected_stdin_does_not_fall_back_to_directory_search() {
    let output = run_with_stdin(&["mini-grep"], b"");

    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn stdin_patterns_search_an_explicit_file_target() {
    let path = std::path::Path::new("tests")
        .join("fixtures")
        .join("text")
        .join("demo.txt");

    let output = run_with_stdin(
        &[
            "-n",
            "-f",
            "-",
            path.to_str().expect("fixture path must be valid Unicode"),
        ],
        b"Rust\r\n",
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let expected = format!(
        "{}:2:Rust makes ownership explicit.\n\
         {}:4:Rust rewards careful design.\n",
        path.display(),
        path.display(),
    );

    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn stdin_patterns_distinguish_empty_input_from_a_blank_pattern() {
    let path = std::path::Path::new("tests")
        .join("fixtures")
        .join("text")
        .join("demo.txt");

    let cases: &[(&[u8], bool, usize)] = &[
        (b"", false, 0),
        (b"\n", false, 4),
        (b"", true, 4),
        (b"\n", true, 0),
    ];

    for &(patterns, invert, expected_count) in cases {
        let mut arguments = vec!["-c", "-f", "-"];

        if invert {
            arguments.push("-v");
        }

        arguments.push(path.to_str().expect("fixture path must be valid Unicode"));

        let output = run_with_stdin(&arguments, patterns);

        assert_eq!(
            output.status.code(),
            Some(if expected_count == 0 { 1 } else { 0 }),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let expected = format!("{}:{expected_count}\n", path.display());
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn stdin_pattern_errors_precede_search_even_with_a_zero_limit() {
    let path = std::path::Path::new("tests")
        .join("fixtures")
        .join("text")
        .join("demo.txt");

    let cases: &[(&[u8], &str)] = &[
        (b"Rust\n\xff\n", "cannot read"),
        (b"Rust\n[\n", "invalid search pattern"),
    ];

    for &(patterns, diagnostic) in cases {
        for zero_limit in [false, true] {
            let mut arguments = vec!["-f", "-"];

            if zero_limit {
                arguments.extend_from_slice(&["--max-results", "0"]);
            }

            arguments.push(path.to_str().expect("fixture path must be valid Unicode"));

            let output = run_with_stdin(&arguments, patterns);

            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());

            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains(diagnostic), "{stderr}");
            assert!(!stderr.contains("panicked at"), "{stderr}");

            if diagnostic == "cannot read" {
                assert!(stderr.contains("(standard input)"), "{stderr}");
            }
        }
    }
}

#[test]
fn stdin_pattern_ownership_conflicts_are_usage_errors() {
    let cases: &[&[&str]] = &[
        &["-f", "-", "-"],
        &["-f", "-", "-f", "-", "tests/fixtures/text/demo.txt"],
    ];

    for arguments in cases {
        let output = run_with_stdin(arguments, b"");

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("error:"), "{stderr}");
        assert!(stderr.contains("stdin"), "{stderr}");
        assert!(stderr.contains("Usage:"), "{stderr}");
    }
}

#[test]
fn stdin_path_lists_supply_filesystem_targets() {
    let path = std::path::Path::new("tests")
        .join("fixtures")
        .join("text")
        .join("demo.txt");
    let contents = format!("{}\r\n", path.display());

    for enumerate in [false, true] {
        let mut arguments = if enumerate {
            vec!["--files"]
        } else {
            vec!["-n", "Rust"]
        };
        arguments.extend_from_slice(&["--files-from", "-"]);

        let output = run_with_stdin(&arguments, contents.as_bytes());

        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());

        let expected = if enumerate {
            format!("{}\n", path.display())
        } else {
            format!(
                "{}:2:Rust makes ownership explicit.\n\
                {}:4:Rust rewards careful design.\n",
                path.display(),
                path.display(),
            )
        };

        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn path_lists_reject_shared_stdin_and_extra_positional_targets() {
    let cases: &[&[&str]] = &[
        &["-f", "-", "--files-from", "-"],
        &["Rust", "--files-from", "-", "--files-from", "-"],
        &["Rust", "extra", "--files-from", "-"],
        &["--files", "extra", "--files-from", "-"],
    ];

    for arguments in cases {
        let output = run_with_stdin(arguments, b"");

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("error:"), "{stderr}");
        assert!(stderr.contains("Usage:"), "{stderr}");
    }
}

#[test]
fn stdin_binary_detection_and_text_override_share_file_behavior() {
    let contents = b"Rust\0ok\n";

    let output = run_with_stdin(&["-n", "Rust", "-"], contents);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let output = run_with_stdin(&["--text", "-n", "Rust", "-"], contents);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"(standard input):1:Rust\0ok\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn stdin_text_override_still_reports_invalid_utf8() {
    let contents = b"\xff\0Rust\n";

    let output = run_with_stdin(&["Rust", "-"], contents);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let output = run_with_stdin(&["--text", "Rust", "-"], contents);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot read"), "{stderr}");
    assert!(stderr.contains("(standard input)"), "{stderr}");
}

#[test]
fn json_stdin_emits_source_identity_and_byte_spans() {
    let output = run_with_stdin(
        &[
            "--json",
            "-n",
            "--column",
            "--heading",
            "--color=always",
            "Rust",
            "-",
        ],
        "é Rust\n".as_bytes(),
    );

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("JSON output must be UTF-8");
    let records: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("record must be valid JSON"))
        .collect();

    assert_eq!(
        records,
        vec![
            serde_json::json!({
                "schema_version": 1,
                "source": { "kind": "stdin" },
                "type": "begin"
            }),
            serde_json::json!({
                "schema_version": 1,
                "source": { "kind": "stdin" },
                "type": "line",
                "line_number": 1,
                "selected": true,
                "text": "é Rust",
                "spans": [{ "start": 3, "end": 7 }]
            })
        ]
    );
    assert!(stdout.ends_with('\n'));
    assert!(!stdout.contains('\u{1b}'));
}

#[test]
fn quiet_json_search_suppresses_all_records() {
    for (contents, expected_status) in [(b"Rust\n".as_slice(), 0), (b"Go\n".as_slice(), 1)] {
        let output = run_with_stdin(&["--json", "-q", "Rust", "-"], contents);

        assert_eq!(output.status.code(), Some(expected_status));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn json_counts_distinguish_selected_lines_from_matches() {
    for (flag, expected_count, expected_unit) in [
        ("-c", "2", "selected_lines"),
        ("--count-matches", "3", "matches"),
    ] {
        let output = run_with_stdin(&["--json", flag, "Rust", "-"], b"Rust Rust\nGo\nRust\n");

        assert_eq!(output.status.code(), Some(0));
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8(output.stdout).expect("JSON output must be UTF-8");
        let records: Vec<serde_json::Value> = stdout
            .lines()
            .map(|line| serde_json::from_str(line).expect("record must be valid JSON"))
            .collect();

        assert_eq!(
            records,
            vec![
                serde_json::json!({
                    "schema_version": 1,
                    "source": { "kind": "stdin" },
                    "type": "begin"
                }),
                serde_json::json!({
                    "schema_version": 1,
                    "source": { "kind": "stdin" },
                    "type": "count",
                    "count": expected_count,
                    "unit": expected_unit
                })
            ],
            "flag: {flag}"
        );
        assert!(stdout.ends_with('\n'));
    }
}

#[test]
fn json_only_matching_preserves_original_byte_offsets() {
    let output = run_with_stdin(&["--json", "-o", "Rust", "-"], "é Rust Rust\n".as_bytes());

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("JSON output must be UTF-8");
    let records: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("record must be valid JSON"))
        .collect();

    assert_eq!(
        records,
        vec![
            serde_json::json!({
                "schema_version": 1,
                "source": { "kind": "stdin" },
                "type": "begin"
            }),
            serde_json::json!({
                "schema_version": 1,
                "source": { "kind": "stdin" },
                "type": "match",
                "line_number": 1,
                "text": "Rust",
                "start": 3,
                "end": 7
            }),
            serde_json::json!({
                "schema_version": 1,
                "source": { "kind": "stdin" },
                "type": "match",
                "line_number": 1,
                "text": "Rust",
                "start": 8,
                "end": 12
            })
        ]
    );
    assert!(stdout.ends_with('\n'));
}

#[test]
fn json_context_marks_selected_lines_and_separates_groups() {
    let output = run_with_stdin(
        &["--json", "-C", "1", "Rust", "-"],
        b"before\nRust\nafter\nskip\nskip\nbefore\nRust\nafter\n",
    );

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("JSON output must be UTF-8");
    let records: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("record must be valid JSON"))
        .collect();

    let types: Vec<_> = records
        .iter()
        .map(|record| {
            record["type"]
                .as_str()
                .expect("record type must be a string")
        })
        .collect();

    assert_eq!(
        types,
        [
            "begin",
            "line",
            "line",
            "line",
            "group_break",
            "line",
            "line",
            "line",
        ]
    );

    let lines: Vec<_> = records
        .iter()
        .filter(|record| record["type"] == "line")
        .map(|record| {
            serde_json::json!({
                "line_number": record["line_number"],
                "selected": record["selected"],
                "text": record["text"],
                "spans": record["spans"]
            })
        })
        .collect();

    let expected: Vec<_> = [
        (1, false, "before"),
        (2, true, "Rust"),
        (3, false, "after"),
        (6, false, "before"),
        (7, true, "Rust"),
        (8, false, "after"),
    ]
    .into_iter()
    .map(|(line_number, selected, text)| {
        let spans = if selected {
            serde_json::json!([{ "start": 0, "end": 4 }])
        } else {
            serde_json::json!([])
        };

        serde_json::json!({
            "line_number": line_number,
            "selected": selected,
            "text": text,
            "spans": spans
        })
    })
    .collect();

    assert_eq!(lines, expected);

    for record in &records {
        assert_eq!(record["schema_version"], 1);
        assert_eq!(record["source"], serde_json::json!({ "kind": "stdin" }));
    }

    assert!(stdout.ends_with('\n'));
}

#[test]
fn json_read_errors_preserve_completed_records_and_use_stderr() {
    let output = run_with_stdin(&["--json", "Rust", "-"], b"Rust\n\xff\n");

    assert_eq!(output.status.code(), Some(2));

    let stdout = String::from_utf8(output.stdout).expect("JSON output must be UTF-8");
    let records: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("record must be valid JSON"))
        .collect();

    assert_eq!(
        records,
        vec![
            serde_json::json!({
                "schema_version": 1,
                "source": { "kind": "stdin" },
                "type": "begin"
            }),
            serde_json::json!({
                "schema_version": 1,
                "source": { "kind": "stdin" },
                "type": "line",
                "line_number": 1,
                "selected": true,
                "text": "Rust",
                "spans": [{ "start": 0, "end": 4 }]
            })
        ]
    );

    let stderr = String::from_utf8(output.stderr).expect("diagnostic must be UTF-8");
    assert!(stderr.contains("mini-grep:"), "{stderr}");
    assert!(stderr.contains("(standard input)"), "{stderr}");
    assert!(stdout.ends_with('\n'));
}

#[test]
fn json_search_without_matches_returns_one_and_only_begin() {
    for contents in [b"Go\n".as_slice(), b"".as_slice()] {
        let output = run_with_stdin(&["--json", "Rust", "-"], contents);

        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());

        let stdout = String::from_utf8(output.stdout).expect("JSON output must be UTF-8");
        let records: Vec<serde_json::Value> = stdout
            .lines()
            .map(|line| serde_json::from_str(line).expect("record must be valid JSON"))
            .collect();

        assert_eq!(
            records,
            vec![serde_json::json!({
                "schema_version": 1,
                "source": { "kind": "stdin" },
                "type": "begin"
            })]
        );
        assert!(stdout.ends_with('\n'));
    }
}

#[test]
fn json_only_matching_skips_zero_width_matches() {
    let output = run_with_stdin(&["--json", "-o", "^", "-"], b"Rust\n");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout).expect("JSON output must be UTF-8");
    let records: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("record must be valid JSON"))
        .collect();

    assert_eq!(
        records,
        vec![serde_json::json!({
            "schema_version": 1,
            "source": { "kind": "stdin" },
            "type": "begin"
        })]
    );
    assert!(stdout.ends_with('\n'));
}

#[test]
fn json_statistics_emit_a_final_run_summary() {
    let output = run_with_stdin(
        &["--json", "--stats", "Rust", "-"],
        b"Rust Rust\nGo\nRust\n",
    );

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("JSON output must be UTF-8");
    let records: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("record must be valid JSON"))
        .collect();

    assert_eq!(records.len(), 4);

    let summary = records.last().expect("summary must be present");
    assert_eq!(summary["schema_version"], 1);
    assert_eq!(summary["type"], "summary");
    assert_eq!(summary["inputs_started"], "1");
    assert_eq!(summary["inputs_with_selected_lines"], "1");
    assert_eq!(summary["selected_lines"], "2");
    assert_eq!(summary["matches"], "3");
    assert_eq!(summary["errors"], "0");
    assert!(summary.get("source").is_none());

    summary["elapsed_nanoseconds"]
        .as_str()
        .expect("elapsed time must be a decimal string")
        .parse::<u128>()
        .expect("elapsed time must be an unsigned integer");

    assert!(stdout.ends_with('\n'));
}

fn parse_json_statistics(output: &Output) -> serde_json::Value {
    let stdout = std::str::from_utf8(&output.stdout).expect("JSON output must be UTF-8");

    assert!(stdout.ends_with('\n'));

    let records: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("record must be valid JSON"))
        .collect();

    assert_eq!(
        records
            .iter()
            .filter(|record| record["type"] == "summary")
            .count(),
        1
    );

    let summary = records.last().expect("summary must be present");
    assert_eq!(summary["schema_version"], 1);
    assert_eq!(summary["type"], "summary");
    assert!(summary.get("source").is_none());

    summary["elapsed_nanoseconds"]
        .as_str()
        .expect("elapsed time must be a decimal string")
        .parse::<u128>()
        .expect("elapsed time must be an unsigned integer");

    summary.clone()
}

#[test]
fn statistics_preserve_partial_progress_without_double_counting_errors() {
    let output = run_with_stdin(&["--json", "--stats", "Rust", "-"], b"Rust\n\xff\n");

    assert_eq!(output.status.code(), Some(2));

    let summary = parse_json_statistics(&output);
    assert_eq!(summary["inputs_started"], "1");
    assert_eq!(summary["inputs_with_selected_lines"], "1");
    assert_eq!(summary["selected_lines"], "1");
    assert_eq!(summary["matches"], "1");
    assert_eq!(summary["errors"], "1");

    let stderr = String::from_utf8(output.stderr).expect("diagnostic must be UTF-8");
    assert!(stderr.contains("mini-grep:"), "{stderr}");
    assert!(stderr.contains("(standard input)"), "{stderr}");
}

#[test]
fn zero_limits_emit_only_an_empty_statistics_summary() {
    for limit in ["-m", "--max-results"] {
        let output = run_with_stdin(
            &["--json", "--stats", limit, "0", "Rust", "-"],
            b"Rust\n\xff\n",
        );

        assert_eq!(output.status.code(), Some(1), "limit: {limit}");
        assert!(output.stderr.is_empty());

        let summary = parse_json_statistics(&output);
        assert_eq!(summary["inputs_started"], "0");
        assert_eq!(summary["inputs_with_selected_lines"], "0");
        assert_eq!(summary["selected_lines"], "0");
        assert_eq!(summary["matches"], "0");
        assert_eq!(summary["errors"], "0");

        let stdout = std::str::from_utf8(&output.stdout).expect("JSON output must be UTF-8");
        assert_eq!(stdout.lines().count(), 1);
    }
}

#[test]
fn terminal_statistics_use_stderr_and_preserve_result_stdout() {
    let output = run_with_stdin(
        &["--stats", "-c", "-h", "Rust", "-"],
        b"Rust Rust\nGo\nRust\n",
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"2\n");

    let stderr = String::from_utf8(output.stderr).expect("summary must be UTF-8");
    let lines: Vec<_> = stderr.lines().collect();

    assert_eq!(lines.len(), 6);
    assert_eq!(
        &lines[..5],
        [
            "inputs_started: 1",
            "inputs_with_selected_lines: 1",
            "selected_lines: 2",
            "matches: 3",
            "errors: 0",
        ]
    );

    let elapsed = lines[5]
        .strip_prefix("elapsed_seconds: ")
        .expect("summary must identify elapsed seconds")
        .parse::<f64>()
        .expect("elapsed seconds must be numeric");

    assert!(elapsed.is_finite());
    assert!(elapsed >= 0.0);
}
