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
        (&["-0c"], "Rust", 0, "(standard input)\01\n"),
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
