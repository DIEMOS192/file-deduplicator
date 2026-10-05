use std::path::{Path, PathBuf};

use crate::cli::{Action, KeepStrategy};
use crate::finder::DuplicateGroup;
use crate::scanner::FileEntry;

/// What to do with one duplicate group.
#[derive(Debug)]
pub struct Plan<'a> {
    /// Files left in place. Never empty.
    pub keep: Vec<&'a FileEntry>,
    pub remove: Vec<&'a FileEntry>,
}

/// Decide which files survive in a group and which are removed.
///
/// `deletable_dirs` limits removal to files inside those directories (empty = anywhere).
/// If any copy lives outside them, all outside copies are kept and every inside copy is
/// removed. Otherwise `strategy` picks the single survivor.
pub fn plan_group<'a>(
    group: &'a DuplicateGroup,
    strategy: KeepStrategy,
    deletable_dirs: &[PathBuf],
) -> Plan<'a> {
    let deletable = |f: &FileEntry| {
        deletable_dirs.is_empty() || deletable_dirs.iter().any(|d| f.path.starts_with(d))
    };
    let (inside, outside): (Vec<&FileEntry>, Vec<&FileEntry>) =
        group.files.iter().partition(|f| deletable(f));
    if !outside.is_empty() {
        return Plan {
            keep: outside,
            remove: inside,
        };
    }

    // `group.files` is sorted by path, and `min_by_key` returns the first minimum,
    // so ties are always broken by path order.
    let keep_index = match strategy {
        KeepStrategy::First => 0,
        KeepStrategy::Oldest => index_of_min(&group.files, |f| f.modified),
        KeepStrategy::Newest => index_of_min(&group.files, |f| std::cmp::Reverse(f.modified)),
        KeepStrategy::ShortestPath => index_of_min(&group.files, |f| f.path.as_os_str().len()),
    };
    Plan {
        keep: vec![&group.files[keep_index]],
        remove: group
            .files
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != keep_index)
            .map(|(_, f)| f)
            .collect(),
    }
}

fn index_of_min<K: Ord>(files: &[FileEntry], key: impl Fn(&FileEntry) -> K) -> usize {
    files
        .iter()
        .enumerate()
        .min_by_key(|(_, f)| key(f))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Remove a single file using the chosen action.
pub fn remove(path: &Path, action: Action) -> Result<(), String> {
    match action {
        Action::Delete => std::fs::remove_file(path).map_err(|e| e.to_string()),
        Action::Trash => trash::delete(path).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn file(path: &str, age_secs: u64) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            size: 10,
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 - age_secs)),
        }
    }

    fn group() -> DuplicateGroup {
        DuplicateGroup {
            size: 10,
            hash: [0; 32],
            files: vec![
                file("/a/long/path/copy.txt", 50),
                file("/b/x.txt", 100),       // oldest
                file("/c/deeper/y.txt", 10), // newest
            ],
        }
    }

    fn paths(files: &[&FileEntry]) -> Vec<String> {
        files
            .iter()
            .map(|f| f.path.to_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn keep_strategies_pick_expected_file() {
        let g = group();
        let kept = |s| paths(&plan_group(&g, s, &[]).keep);
        assert_eq!(kept(KeepStrategy::First), ["/a/long/path/copy.txt"]);
        assert_eq!(kept(KeepStrategy::Oldest), ["/b/x.txt"]);
        assert_eq!(kept(KeepStrategy::Newest), ["/c/deeper/y.txt"]);
        assert_eq!(kept(KeepStrategy::ShortestPath), ["/b/x.txt"]);
    }

    #[test]
    fn plan_removes_all_but_one() {
        let g = group();
        let plan = plan_group(&g, KeepStrategy::Oldest, &[]);
        assert_eq!(plan.keep.len(), 1);
        assert_eq!(plan.remove.len(), 2);
        assert!(plan.remove.iter().all(|f| f.path != plan.keep[0].path));
    }

    #[test]
    fn deletable_dirs_protect_copies_outside_them() {
        let g = group();
        // Only /a and /c are deletable: /b/x.txt is kept regardless of --keep newest.
        let dirs = [PathBuf::from("/a"), PathBuf::from("/c")];
        let plan = plan_group(&g, KeepStrategy::Newest, &dirs);
        assert_eq!(paths(&plan.keep), ["/b/x.txt"]);
        assert_eq!(
            paths(&plan.remove),
            ["/a/long/path/copy.txt", "/c/deeper/y.txt"]
        );
    }

    #[test]
    fn deletable_dirs_keep_every_outside_copy() {
        let g = group();
        let plan = plan_group(&g, KeepStrategy::Oldest, &[PathBuf::from("/c")]);
        assert_eq!(paths(&plan.keep), ["/a/long/path/copy.txt", "/b/x.txt"]);
        assert_eq!(paths(&plan.remove), ["/c/deeper/y.txt"]);
    }

    #[test]
    fn group_entirely_inside_deletable_dir_uses_strategy() {
        let g = group();
        let plan = plan_group(&g, KeepStrategy::Oldest, &[PathBuf::from("/")]);
        assert_eq!(paths(&plan.keep), ["/b/x.txt"]);
        assert_eq!(plan.remove.len(), 2);
    }

    #[test]
    fn group_entirely_outside_removes_nothing() {
        let g = group();
        let plan = plan_group(&g, KeepStrategy::Oldest, &[PathBuf::from("/elsewhere")]);
        assert_eq!(plan.keep.len(), 3);
        assert!(plan.remove.is_empty());
    }
}
