use std::process::Command;

#[test]
fn executable_prints_the_in_memory_search_results() {
    let output = Command::new(env!("CARGO_BIN_EXE_mini-grep"))
        .output()
        .expect("the demo executable should start");

    assert!(
        output.status.success(),
        "unexpected failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    assert_eq!(
        output.stdout,
        b"2:Rust makes ownership explicit.\n4:Rust rewards careful design.\n"
    );
}
