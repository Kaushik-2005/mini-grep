mod filters;
mod ignore;

pub(crate) mod walker;

#[derive(Debug, Clone)]
pub(crate) struct DiscoveryOptions {
    pub(crate) include_hidden: bool,
    pub(crate) ignore_files: bool,
    pub(crate) vcs_ignore_files: bool,
    pub(crate) max_depth: Option<usize>,
    pub(crate) max_filesize: Option<u64>,
    pub(crate) globs: Vec<String>,
    pub(crate) file_types: Vec<String>,
    pub(crate) excluded_types: Vec<String>,
    pub(crate) follow_links: bool,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            ignore_files: true,
            vcs_ignore_files: true,
            max_depth: None,
            max_filesize: None,
            globs: Vec::new(),
            file_types: Vec::new(),
            excluded_types: Vec::new(),
            follow_links: false,
        }
    }
}
