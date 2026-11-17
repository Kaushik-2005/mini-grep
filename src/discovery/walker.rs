use super::DiscoveryOptions;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

pub(crate) fn files(
    root: &Path,
    options: &DiscoveryOptions,
) -> Result<impl Iterator<Item = Result<PathBuf, ignore::Error>>, ignore::Error> {
    let literal_dash = root == Path::new("-");
    let walk_root = if literal_dash {
        Path::new(".").join(root)
    } else {
        root.to_path_buf()
    };
    let root = walk_root.as_path();

    if !options.follow_links
        && std::fs::symlink_metadata(root).is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(ignore::Error::WithPath {
            path: root.to_path_buf(),
            err: Box::new(ignore::Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "symbolic link root requires --follow",
            ))),
        });
    }

    let overrides = if options.globs.is_empty() {
        ignore::overrides::Override::empty()
    } else {
        let glob_root = if root.is_file() {
            root.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
        } else {
            root
        };

        super::filters::compile_globs(glob_root, &options.globs)?
    };

    let types = super::filters::compile_types(&options.file_types, &options.excluded_types)?;

    let mut builder = WalkBuilder::new(root);

    builder
        .standard_filters(false)
        .hidden(!options.include_hidden)
        .follow_links(options.follow_links)
        .overrides(overrides.clone());

    super::ignore::configure(&mut builder, options);
    super::filters::configure(&mut builder, options);

    let mut entries = builder.build();
    let mut pending = VecDeque::new();

    Ok(std::iter::from_fn(move || {
        loop {
            if let Some(event) = pending.pop_front() {
                return Some(event);
            }

            let entry = match entries.next()? {
                Ok(entry) => entry,
                Err(error) => return Some(Err(error)),
            };

            let file_type = entry.file_type();

            if file_type.is_some_and(|kind| kind.is_dir()) {
                pending.extend(super::ignore::read_errors(entry.path(), options).map(Err));
            }

            if let Some(error) = entry.error() {
                pending.push_back(Err(error.clone()));
            }

            if types.matched(entry.path(), false).is_ignore() {
                continue;
            }

            if file_type.is_some_and(|kind| kind.is_file()) {
                // The dependency bypasses overrides for explicit roots.
                if overrides.matched(entry.path(), false).is_ignore() {
                    continue;
                }

                match super::filters::within_size_limit(&entry, options.max_filesize) {
                    Ok(true) => {
                        let path = entry.into_path();

                        let path = if literal_dash {
                            match path.strip_prefix(".") {
                                Ok(relative) => relative.to_path_buf(),
                                Err(_) => path,
                            }
                        } else {
                            path
                        };

                        pending.push_back(Ok(path));
                    }
                    Ok(false) => {}
                    Err(error) => pending.push_back(Err(error)),
                }
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock must be after the Unix epoch")
                .as_nanos();

            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mini-grep-discovery-{}-{timestamp}-{id}",
                std::process::id()
            ));

            fs::create_dir(&path).expect("temporary directory must be created");

            Self { path }
        }

        fn write(&self, relative: &str, contents: &[u8]) -> PathBuf {
            let path = self.path.join(relative);

            fs::create_dir_all(path.parent().expect("fixture file must have a parent"))
                .expect("fixture directories must be created");

            fs::write(&path, contents).expect("fixture file must be written");

            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn discovers_regular_files_in_nested_directories() {
        let directory = TestDirectory::new();

        let first = directory.write("first.txt", b"Rust\n");
        let second = directory.write("nested/second.txt", b"Go\n");
        let third = directory.write("nested/deeper/third.bin", &[0xff]);

        fs::create_dir(directory.path.join("empty"))
            .expect("empty fixture directory must be created");

        let mut actual = files(&directory.path, &DiscoveryOptions::default())
            .expect("valid glob configuration")
            .collect::<Result<Vec<_>, _>>()
            .expect("fixture discovery must succeed");

        let mut expected = vec![first, second, third];

        actual.sort();
        expected.sort();

        assert_eq!(actual, expected);
    }

    #[test]
    fn empty_directory_yields_no_candidates() {
        let directory = TestDirectory::new();

        assert!(
            files(&directory.path, &DiscoveryOptions::default())
                .expect("valid glob configuration")
                .next()
                .is_none()
        );
    }

    #[test]
    fn explicit_regular_file_yields_one_candidate() {
        let directory = TestDirectory::new();
        let path = directory.write("sample file.txt", b"Rust\n");

        let actual = files(&path, &DiscoveryOptions::default())
            .expect("valid glob configuration")
            .collect::<Result<Vec<_>, _>>()
            .expect("explicit file discovery must succeed");

        assert_eq!(actual, vec![path]);
    }

    #[test]
    fn missing_root_preserves_the_traversal_error() {
        let directory = TestDirectory::new();
        let missing = directory.path.join("missing");

        let result = files(&missing, &DiscoveryOptions::default())
            .expect("valid glob configuration")
            .collect::<Result<Vec<_>, _>>();

        let error = result.expect_err("a missing root must fail");
        let message = error.to_string();
        let displayed_path = missing.display().to_string();

        assert!(message.contains(displayed_path.as_str()), "{message}");
    }

    #[test]
    fn zero_size_limit_keeps_only_empty_files() {
        let directory = TestDirectory::new();
        let empty = directory.write("empty.txt", b"");
        directory.write("nonempty.txt", b"Rust\n");

        let options = DiscoveryOptions {
            max_filesize: Some(0),
            ..DiscoveryOptions::default()
        };

        let actual = files(&directory.path, &options)
            .expect("valid glob configuration")
            .collect::<Result<Vec<_>, _>>()
            .expect("fixture discovery must succeed");

        assert_eq!(actual, vec![empty]);
    }
}
