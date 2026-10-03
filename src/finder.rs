use std::collections::HashMap;
use std::hash::Hash as StdHash;
use std::io;
use std::path::Path;

use rayon::prelude::*;

use crate::hasher::{self, Hash};
use crate::scanner::FileEntry;

/// A set of two or more files with identical contents.
#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub size: u64,
    pub hash: Hash,
    /// Sorted by path for stable output.
    pub files: Vec<FileEntry>,
}

impl DuplicateGroup {
    /// Bytes that would be freed by keeping only one copy.
    pub fn wasted_bytes(&self) -> u64 {
        self.size * (self.files.len() as u64 - 1)
    }
}

/// Output of [`find_duplicates`]: the groups plus files that couldn't be read.
#[derive(Debug, Default)]
pub struct FindResult {
    pub groups: Vec<DuplicateGroup>,
    pub errors: Vec<String>,
}

/// Find groups of identical files using a size → partial hash → full hash pipeline.
pub fn find_duplicates(files: Vec<FileEntry>) -> FindResult {
    let mut errors = Vec::new();

    // Stage 1: files with a unique size cannot be duplicates.
    let by_size = group_by(files, |f| f.size);

    // Stage 2: compare a hash of the first few KiB.
    let candidates: Vec<FileEntry> = by_size.into_iter().flatten().collect();
    let by_partial = hash_and_group(candidates, hasher::partial_hash, &mut errors);

    // Stage 3: confirm with a hash of the full contents.
    let candidates: Vec<FileEntry> = by_partial.into_iter().flatten().map(|(f, _)| f).collect();
    let by_full = hash_and_group(candidates, hasher::full_hash, &mut errors);

    let mut groups: Vec<DuplicateGroup> = by_full
        .into_iter()
        .map(|mut files| {
            files.sort_by(|a, b| a.0.path.cmp(&b.0.path));
            DuplicateGroup {
                size: files[0].0.size,
                hash: files[0].1,
                files: files.into_iter().map(|(f, _)| f).collect(),
            }
        })
        .collect();

    // Biggest savings first; ties broken by path so output is deterministic.
    groups.sort_by(|a, b| {
        b.wasted_bytes()
            .cmp(&a.wasted_bytes())
            .then_with(|| a.files[0].path.cmp(&b.files[0].path))
    });

    FindResult { groups, errors }
}

/// Hash every file in parallel (keyed by size too, so different-size files never mix)
/// and return only the groups with more than one member.
fn hash_and_group(
    files: Vec<FileEntry>,
    hash_fn: fn(&Path) -> io::Result<Hash>,
    errors: &mut Vec<String>,
) -> Vec<Vec<(FileEntry, Hash)>> {
    let hashed: Vec<Result<(FileEntry, Hash), String>> = files
        .into_par_iter()
        .map(|f| match hash_fn(&f.path) {
            Ok(h) => Ok((f, h)),
            Err(err) => Err(format!("{}: {}", f.path.display(), err)),
        })
        .collect();

    let mut ok = Vec::with_capacity(hashed.len());
    for item in hashed {
        match item {
            Ok(pair) => ok.push(pair),
            Err(e) => errors.push(e),
        }
    }
    group_by(ok, |(f, h)| (f.size, *h))
}

/// Group items by key, dropping groups with only one member.
fn group_by<T, K: Eq + StdHash>(items: Vec<T>, key: impl Fn(&T) -> K) -> Vec<Vec<T>> {
    let mut map: HashMap<K, Vec<T>> = HashMap::new();
    for item in items {
        map.entry(key(&item)).or_default().push(item);
    }
    map.into_values().filter(|g| g.len() > 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn entry(path: &Path) -> FileEntry {
        let meta = fs::metadata(path).unwrap();
        FileEntry {
            path: path.to_path_buf(),
            size: meta.len(),
            modified: meta.modified().ok(),
        }
    }

    #[test]
    fn groups_identical_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, data: &[u8]| {
            let p = dir.path().join(name);
            fs::write(&p, data).unwrap();
            entry(&p)
        };
        let files = vec![
            write("a1", b"same content"),
            write("a2", b"same content"),
            write("a3", b"same content"),
            write("b", b"diff content"), // same size, different bytes
            write("c", b"unique"),
        ];

        let result = find_duplicates(files);
        assert!(result.errors.is_empty());
        assert_eq!(result.groups.len(), 1);
        let group = &result.groups[0];
        assert_eq!(group.files.len(), 3);
        assert_eq!(group.wasted_bytes(), 2 * 12);
    }

    #[test]
    fn no_duplicates_yields_no_groups() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("x");
        let p2 = dir.path().join("y");
        fs::write(&p1, "one").unwrap();
        fs::write(&p2, "two!").unwrap();
        let result = find_duplicates(vec![entry(&p1), entry(&p2)]);
        assert!(result.groups.is_empty());
    }
}
