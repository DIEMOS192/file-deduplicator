use std::path::Path;

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

/// Directory names skipped unless `--no-default-excludes` is given: dependency caches,
/// build output, VCS metadata and OS trash, where duplicates are expected and needed.
pub const DEFAULT_EXCLUDES: &[&str] = &[
    "node_modules",
    "bower_components",
    ".git",
    ".svn",
    ".hg",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    ".tox",
    "$RECYCLE.BIN",
    "System Volume Information",
];

/// Compiled exclude patterns.
///
/// Patterns without a `/` match a single file or directory name (`node_modules`, `*.tmp`).
/// Patterns with a `/` match the whole path (`**/Photos/raw/**`), using `/` as the
/// separator on every platform.
#[derive(Debug)]
pub struct Excludes {
    names: GlobSet,
    paths: GlobSet,
}

impl Excludes {
    pub fn new(patterns: &[String], use_defaults: bool) -> Result<Self> {
        let mut names = GlobSetBuilder::new();
        let mut paths = GlobSetBuilder::new();
        let defaults = DEFAULT_EXCLUDES.iter().filter(|_| use_defaults).copied();

        for pattern in defaults.chain(patterns.iter().map(String::as_str)) {
            let normalized = pattern.replace('\\', "/");
            let glob = GlobBuilder::new(&normalized)
                .case_insensitive(cfg!(windows))
                .literal_separator(true)
                .backslash_escape(false)
                .build()
                .with_context(|| format!("invalid --exclude pattern `{pattern}`"))?;
            if normalized.contains('/') {
                paths.add(glob);
            } else {
                names.add(glob);
            }
        }

        Ok(Self {
            names: names.build()?,
            paths: paths.build()?,
        })
    }

    pub fn is_excluded(&self, path: &Path) -> bool {
        if let Some(name) = path.file_name() {
            if self.names.is_match(name) {
                return true;
            }
        }
        !self.paths.is_empty() && self.paths.is_match(normalize(path))
    }
}

/// Forward-slash path without the Windows `\\?\` prefix, for path-pattern matching.
fn normalize(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    s.strip_prefix("//?/").map(str::to_owned).unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn excludes(patterns: &[&str], defaults: bool) -> Excludes {
        let patterns: Vec<String> = patterns.iter().map(|s| s.to_string()).collect();
        Excludes::new(&patterns, defaults).unwrap()
    }

    #[test]
    fn name_patterns_match_any_component_name() {
        let ex = excludes(&["*.tmp", "cache"], false);
        assert!(ex.is_excluded(Path::new("/home/me/file.tmp")));
        assert!(ex.is_excluded(Path::new("/home/me/cache")));
        assert!(!ex.is_excluded(Path::new("/home/me/cache.txt")));
        assert!(!ex.is_excluded(Path::new("/home/me/file.txt")));
    }

    #[test]
    fn path_patterns_match_full_path() {
        let ex = excludes(&["**/Photos/raw/**"], false);
        assert!(ex.is_excluded(Path::new("/home/me/Photos/raw/a.cr2")));
        assert!(!ex.is_excluded(Path::new("/home/me/Photos/a.jpg")));
    }

    #[test]
    fn defaults_can_be_disabled() {
        assert!(excludes(&[], true).is_excluded(Path::new("/p/node_modules")));
        assert!(!excludes(&[], false).is_excluded(Path::new("/p/node_modules")));
    }

    #[test]
    fn invalid_pattern_is_an_error() {
        assert!(Excludes::new(&["[".to_string()], false).is_err());
    }
}
