use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::Result;
use walkdir::{DirEntry, WalkDir};

use crate::cli::ScanOptions;
use crate::exclude::Excludes;

/// A regular file discovered while walking the given paths.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// Result of a directory walk: the files found plus any non-fatal errors.
#[derive(Debug, Default)]
pub struct ScanResult {
    pub files: Vec<FileEntry>,
    pub errors: Vec<String>,
}

/// Walk every path in `options.paths` and collect files that pass the filters.
///
/// Unreadable entries are recorded in `errors` instead of aborting the scan.
/// The same file reached through two overlapping paths is only listed once.
/// Fails only if an exclude pattern is invalid.
pub fn scan(options: &ScanOptions) -> Result<ScanResult> {
    let excludes = Excludes::new(&options.exclude, !options.no_default_excludes)?;
    let mut result = ScanResult::default();
    let mut seen = std::collections::HashSet::new();

    for root in &options.paths {
        let walker = WalkDir::new(root)
            .follow_links(options.follow_links)
            .into_iter()
            // Never filter out the root itself, even if it looks hidden (e.g. ".") or excluded.
            .filter_entry(|e| {
                e.depth() == 0
                    || ((options.hidden || !is_hidden(e)) && !excludes.is_excluded(e.path()))
            });

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    result.errors.push(err.to_string());
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(err) => {
                    result.errors.push(err.to_string());
                    continue;
                }
            };
            if metadata.len() < options.min_size {
                continue;
            }
            let path =
                std::fs::canonicalize(entry.path()).unwrap_or_else(|_| entry.path().to_path_buf());
            if !seen.insert(path.clone()) {
                continue;
            }
            result.files.push(FileEntry {
                path,
                size: metadata.len(),
                modified: metadata.modified().ok(),
            });
        }
    }

    Ok(result)
}

fn is_hidden(entry: &DirEntry) -> bool {
    if entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.'))
    {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if let Ok(meta) = entry.metadata() {
            return meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn options(root: &std::path::Path) -> ScanOptions {
        ScanOptions {
            paths: vec![root.to_path_buf()],
            min_size: 1,
            hidden: false,
            follow_links: false,
            exclude: Vec::new(),
            no_default_excludes: false,
        }
    }

    #[test]
    fn finds_files_and_applies_filters() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        fs::write(dir.path().join("empty.txt"), "").unwrap();
        fs::create_dir(dir.path().join(".hidden")).unwrap();
        fs::write(dir.path().join(".hidden").join("b.txt"), "hello").unwrap();

        let result = scan(&options(dir.path())).unwrap();
        assert_eq!(result.files.len(), 1, "empty and hidden files skipped");

        let mut opts = options(dir.path());
        opts.hidden = true;
        assert_eq!(scan(&opts).unwrap().files.len(), 2);

        opts.min_size = 0;
        assert_eq!(scan(&opts).unwrap().files.len(), 3);
    }

    #[test]
    fn excluded_directories_are_pruned() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("node_modules").join("pkg")).unwrap();
        fs::write(
            dir.path().join("node_modules").join("pkg").join("a.js"),
            "x",
        )
        .unwrap();
        fs::write(dir.path().join("keep.txt"), "x").unwrap();
        fs::write(dir.path().join("skip.tmp"), "x").unwrap();

        let mut opts = options(dir.path());
        opts.exclude = vec!["*.tmp".into()];
        assert_eq!(scan(&opts).unwrap().files.len(), 1, "defaults + *.tmp");

        opts.no_default_excludes = true;
        assert_eq!(scan(&opts).unwrap().files.len(), 2, "node_modules included");
    }

    #[test]
    fn overlapping_paths_are_not_double_counted() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let mut opts = options(dir.path());
        opts.paths.push(dir.path().join("a.txt"));
        assert_eq!(scan(&opts).unwrap().files.len(), 1);
    }
}
