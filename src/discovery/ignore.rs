use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use super::DiscoveryOptions;

use ::ignore::{Error, WalkBuilder};

pub(super) fn configure(builder: &mut WalkBuilder, options: &DiscoveryOptions) {
    builder
        .ignore(options.ignore_files)
        .git_ignore(options.vcs_ignore_files)
        .git_exclude(options.vcs_ignore_files)
        .require_git(false)
        .parents(false)
        .git_global(false);
}

pub(super) fn read_errors<'a>(
    directory: &'a Path,
    options: &'a DiscoveryOptions,
) -> impl Iterator<Item = Error> + 'a {
    [
        (".ignore", options.ignore_files),
        (".gitignore", options.vcs_ignore_files),
        (".git/info/exclude", options.vcs_ignore_files),
    ]
    .into_iter()
    .filter_map(move |(relative, enabled)| {
        if !enabled {
            return None;
        }

        let path = relative
            .split('/')
            .fold(directory.to_path_buf(), |path, part| path.join(part));

        let result = match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => check_readable(&path),
            Ok(_) => return None,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return None;
            }
            Err(error) => Err(error),
        };

        result.err().map(|source| Error::WithPath {
            path,
            err: Box::new(Error::Io(source)),
        })
    })
}

fn check_readable(path: &Path) -> io::Result<()> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();

    loop {
        line.clear();

        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
    }
}
