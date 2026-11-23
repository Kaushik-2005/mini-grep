use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::Path;

use crate::AppError;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ColorSetting {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SortSetting {
    None,
    Path,
}

/// Optional defaults supplied by local configuration file
///
/// Missing fields leave the corresponding built-in default unchanged.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct FileConfig {
    pub(crate) threads: Option<NonZeroUsize>,
    pub(crate) color: Option<ColorSetting>,
    pub(crate) sort: Option<SortSetting>,
}

/// Parses a JSON object without accessing the filesystem.
pub(crate) fn parse(contents: &str) -> Result<FileConfig, serde_json::Error> {
    if !contents.trim_start().starts_with('{') {
        return Err(<serde_json::Error as serde::de::Error>::custom(
            "configuration must be a JSON object",
        ));
    }

    serde_json::from_str(contents)
}

/// Loads an optional configuration file.
///
/// Only a missing file is treated as an empty configuration.
pub(crate) fn load(path: &Path) -> Result<FileConfig, AppError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(FileConfig::default());
        }
        Err(source) => {
            return Err(AppError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    parse(&contents).map_err(|source| AppError::ConfigParse {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) const FILE_NAME: &str = ".mini-grep.json";

#[cfg(test)]
mod tests {
    use super::*;

    use std::error::Error as _;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct ConfigFixture {
        directory: PathBuf,
    }

    impl ConfigFixture {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock must be after the Unix epoch")
                .as_nanos();
            let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "mini-grep-config-{}-{timestamp}-{id}",
                std::process::id()
            ));

            fs::create_dir(&directory).expect("fixture directory must be created");

            Self { directory }
        }

        fn path(&self) -> PathBuf {
            self.directory.join(".mini-grep.json")
        }
    }

    impl Drop for ConfigFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn loads_existing_settings_and_allows_a_missing_file() {
        let fixture = ConfigFixture::new();
        let path = fixture.path();

        assert_eq!(
            load(&path).expect("missing configuration must be allowed"),
            FileConfig::default()
        );

        fs::write(&path, r#"{"threads": 4, "color": "never"}"#)
            .expect("fixture configuration must be written");

        let config = load(&path).expect("valid configuration must load");

        assert_eq!(config.threads, NonZeroUsize::new(4));
        assert_eq!(config.color, Some(ColorSetting::Never));
        assert_eq!(config.sort, None);
    }

    #[test]
    fn parse_failures_preserve_path_and_underlying_source() {
        let fixture = ConfigFixture::new();
        let path = fixture.path();

        fs::write(&path, r#"{"threads": 0}"#).expect("fixture configuration must be written");

        let error = load(&path).expect_err("invalid configuration must fail");

        match &error {
            AppError::ConfigParse {
                path: error_path,
                source,
            } => {
                assert_eq!(error_path, &path);
                assert!(source.is_data());
            }
            other => panic!("expected configuration parse error, got {other:?}"),
        }

        assert!(error.source().is_some());

        let diagnostic = error.to_string();
        assert!(diagnostic.contains("cannot parse configuration"));
        assert!(diagnostic.contains(path.to_string_lossy().as_ref()));
    }

    #[test]
    fn directories_and_invalid_utf8_are_read_errors() {
        let fixture = ConfigFixture::new();
        let path = fixture.path();

        fs::write(&path, b"\xff").expect("invalid UTF-8 fixture must be written");

        for target in [&fixture.directory, &path] {
            let error = load(target).expect_err("unreadable configuration must fail");

            match &error {
                AppError::Read {
                    path: error_path, ..
                } => {
                    assert_eq!(error_path, target);
                }
                other => panic!("expected read error, got {other:?}"),
            }

            assert!(error.source().is_some());
        }
    }

    #[test]
    fn empty_object_leaves_all_defaults_unspecified() {
        let config = parse("{}").expect("empty configuration must parse");

        assert_eq!(config, FileConfig::default());
    }

    #[test]
    fn parses_supported_settings() {
        let config = parse(
            r#"{
                "threads": 4,
                "color": "never",
                "sort": "path"
            }"#,
        )
        .expect("supported settings must parse");

        assert_eq!(
            config,
            FileConfig {
                threads: NonZeroUsize::new(4),
                color: Some(ColorSetting::Never),
                sort: Some(SortSetting::Path),
            }
        );
    }

    #[test]
    fn preserves_explicit_values_that_equal_builtin_defaults() {
        let config = parse(
            r#"{
                "threads": 1,
                "color": "auto",
                "sort": "none"
            }"#,
        )
        .expect("explicit default values must parse");

        assert_eq!(config.threads, NonZeroUsize::new(1));
        assert_eq!(config.color, Some(ColorSetting::Auto));
        assert_eq!(config.sort, Some(SortSetting::None));
    }

    #[test]
    fn rejects_unknown_and_duplicate_fields() {
        for contents in [
            r#"{"thread": 4}"#,
            r#"{"threads": 2, "threads": 4}"#,
            r#"{"color": "auto", "color": "never"}"#,
        ] {
            assert!(
                parse(contents).is_err(),
                "invalid configuration was accepted: {contents}"
            );
        }
    }

    #[test]
    fn rejects_invalid_setting_values() {
        for contents in [
            r#"{"threads": 0}"#,
            r#"{"threads": -1}"#,
            r#"{"threads": 1.5}"#,
            r#"{"threads": "4"}"#,
            r#"{"color": "sometimes"}"#,
            r#"{"color": "Always"}"#,
            r#"{"sort": "size"}"#,
            r#"{"sort": true}"#,
        ] {
            assert!(
                parse(contents).is_err(),
                "invalid configuration was accepted: {contents}"
            );
        }
    }

    #[test]
    fn rejects_non_objects_malformed_json_and_trailing_data() {
        for contents in [
            "",
            "null",
            "[]",
            "42",
            r#""configuration""#,
            "{",
            r#"{"threads": 2,}"#,
            "{} {}",
        ] {
            assert!(
                parse(contents).is_err(),
                "invalid configuration was accepted: {contents}"
            );
        }
    }
}
