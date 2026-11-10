use std::path::Path;

use ::ignore::overrides::{Override, OverrideBuilder};
use ::ignore::types::{Types, TypesBuilder};
use ::ignore::{DirEntry, Error, WalkBuilder};

use super::DiscoveryOptions;

pub(super) fn configure(builder: &mut WalkBuilder, options: &DiscoveryOptions) {
    builder.max_depth(options.max_depth);
}

pub(super) fn within_size_limit(entry: &DirEntry, limit: Option<u64>) -> Result<bool, Error> {
    match limit {
        Some(limit) => Ok(entry.metadata()?.len() <= limit),
        None => Ok(true),
    }
}

pub(super) fn compile_globs(root: &Path, patterns: &[String]) -> Result<Override, Error> {
    let mut builder = OverrideBuilder::new(root);

    for pattern in patterns {
        builder.add(pattern)?;
    }

    builder.build()
}

pub(super) fn compile_types(included: &[String], excluded: &[String]) -> Result<Types, Error> {
    if included.is_empty() && excluded.is_empty() {
        return Ok(Types::empty());
    }

    let mut builder = TypesBuilder::new();
    builder.add_defaults();

    for name in included {
        builder.select(name);
    }

    // Adding exclusions last gives them precedence.
    for name in excluded {
        builder.negate(name);
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile(patterns: &[&str]) -> Override {
        let patterns: Vec<String> = patterns
            .iter()
            .map(|pattern| (*pattern).to_owned())
            .collect();

        compile_globs(Path::new("project"), &patterns).expect("valid globs must compile")
    }

    #[test]
    fn no_globs_leave_paths_undecided() {
        let matcher = compile(&[]);

        assert!(matcher.matched("main.rs", false).is_none());
        assert!(matcher.matched("notes.txt", false).is_none());
    }

    #[test]
    fn positive_globs_include_matching_files_only() {
        let matcher = compile(&["*.rs", "*.toml"]);

        assert!(matcher.matched("src/main.rs", false).is_whitelist());
        assert!(matcher.matched("Cargo.toml", false).is_whitelist());
        assert!(matcher.matched("README.md", false).is_ignore());
    }

    #[test]
    fn negative_globs_exclude_without_restricting_other_files() {
        let matcher = compile(&["!*.log"]);

        assert!(matcher.matched("debug.log", false).is_ignore());
        assert!(matcher.matched("notes.txt", false).is_none());
    }

    #[test]
    fn last_matching_glob_wins() {
        let excluded = compile(&["*.rs", "!generated.rs"]);

        assert!(excluded.matched("generated.rs", false).is_ignore());

        let included = compile(&["*.rs", "!generated.rs", "generated.rs"]);

        assert!(included.matched("generated.rs", false).is_whitelist());
    }

    #[test]
    fn anchored_globs_are_relative_to_the_root() {
        let matcher = compile(&["/src/*.rs"]);

        assert!(matcher.matched("src/main.rs", false).is_whitelist());
        assert!(matcher.matched("tests/src/main.rs", false).is_ignore());
        assert!(matcher.matched("src/nested/main.rs", false).is_ignore());
    }

    #[test]
    fn glob_matching_is_case_sensitive() {
        let matcher = compile(&["*.rs"]);

        assert!(matcher.matched("main.rs", false).is_whitelist());
        assert!(matcher.matched("MAIN.RS", false).is_ignore());
    }

    #[test]
    fn invalid_globs_return_an_error() {
        let patterns = vec![String::from("[z-a]")];

        let error =
            compile_globs(Path::new("project"), &patterns).expect_err("an invalid range must fail");

        assert!(error.to_string().contains("[z-a]"));
    }

    #[test]
    fn no_type_filters_leave_paths_undecided() {
        let matcher = compile_types(&[], &[]).expect("empty type configuration must compile");

        assert!(matcher.matched("main.rs", false).is_none());
        assert!(matcher.matched("notes.txt", false).is_none());
    }

    #[test]
    fn type_inclusions_form_a_union() {
        let included = vec![String::from("rust"), String::from("toml")];

        let matcher = compile_types(&included, &[]).expect("known types must compile");

        assert!(matcher.matched("main.rs", false).is_whitelist());
        assert!(matcher.matched("Cargo.toml", false).is_whitelist());
        assert!(matcher.matched("Cargo.lock", false).is_whitelist());
        assert!(matcher.matched("notes.txt", false).is_ignore());
    }

    #[test]
    fn type_exclusions_win_and_do_not_require_inclusions() {
        let rust = vec![String::from("rust")];

        let excluded_only = compile_types(&[], &rust).expect("known exclusions must compile");

        assert!(excluded_only.matched("main.rs", false).is_ignore());
        assert!(excluded_only.matched("notes.txt", false).is_none());

        let both = compile_types(&rust, &rust).expect("compatible type settings must compile");

        assert!(both.matched("main.rs", false).is_ignore());
    }

    #[test]
    fn unknown_type_names_return_errors() {
        let unknown = vec![String::from("mini-grep-unknown-type")];

        assert!(compile_types(&unknown, &[]).is_err());
        assert!(compile_types(&[], &unknown).is_err());
    }
}
