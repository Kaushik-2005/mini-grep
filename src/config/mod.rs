pub(crate) mod file;

use std::num::NonZeroUsize;

use serde::Serialize;

use file::{ColorSetting, FileConfig, SortSetting};

/// Configuration defaults after applying the optional local file.
///
/// Explicit search arguments are applied separately by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct ResolvedConfig {
    pub(crate) threads: NonZeroUsize,
    pub(crate) color: ColorSetting,
    pub(crate) sort: SortSetting,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            threads: NonZeroUsize::MIN,
            color: ColorSetting::Auto,
            sort: SortSetting::None,
        }
    }
}

impl From<FileConfig> for ResolvedConfig {
    fn from(config: FileConfig) -> Self {
        let defaults = Self::default();

        Self {
            threads: config.threads.unwrap_or(defaults.threads),
            color: config.color.unwrap_or(defaults.color),
            sort: config.sort.unwrap_or(defaults.sort),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_settings_resolve_to_builtin_defaults() {
        let resolved = ResolvedConfig::from(FileConfig::default());

        assert_eq!(resolved, ResolvedConfig::default());
        assert_eq!(resolved.threads.get(), 1);
        assert_eq!(resolved.color, ColorSetting::Auto);
        assert_eq!(resolved.sort, SortSetting::None);
    }

    #[test]
    fn specified_settings_replace_only_their_own_defaults() {
        let resolved = ResolvedConfig::from(FileConfig {
            threads: NonZeroUsize::new(4),
            color: None,
            sort: Some(SortSetting::Path),
        });

        assert_eq!(resolved.threads.get(), 4);
        assert_eq!(resolved.color, ColorSetting::Auto);
        assert_eq!(resolved.sort, SortSetting::Path);
    }

    #[test]
    fn resolved_defaults_serialize_as_concrete_settings() {
        let resolved = ResolvedConfig::from(FileConfig {
            threads: NonZeroUsize::new(2),
            color: Some(ColorSetting::Never),
            sort: Some(SortSetting::Path),
        });

        let value = serde_json::to_value(resolved).expect("resolved settings must serialize");

        assert_eq!(
            value,
            serde_json::json!({
                "threads": 2,
                "color": "never",
                "sort": "path"
            })
        );
    }
}
