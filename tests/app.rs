use std::process::Command;

#[test]
fn executable_searches_a_file() {
    let path = "tests/fixtures/text/demo.txt";

    let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["-n", "Rust", path])
        .output()
        .expect("the executable should start");

    assert_eq!(
        output.status.code(),
        Some(0),
        "unexpected failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let expected = format!(
        "{path}:2:Rust makes ownership explicit.\n\
        {path}:4:Rust rewards careful design.\n"
    );

    assert_eq!(output.stdout, expected.as_bytes());
}
