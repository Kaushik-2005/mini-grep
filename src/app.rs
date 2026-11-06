use std::io;

use crate::{output::terminal, search};

pub fn run() -> io::Result<()> {
    let text = include_str!("../tests/fixtures/text/demo.txt");
    let matches = search::search("Rust", text);

    let stdout = io::stdout();
    let mut writer = stdout.lock();

    terminal::write_matches(&mut writer, &matches)
}
