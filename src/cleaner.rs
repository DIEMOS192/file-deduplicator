use std::path::Path;

use crate::cli::{Action, KeepStrategy};
use crate::finder::DuplicateGroup;
use crate::scanner::FileEntry;

/// What to do with one duplicate group.
#[derive(Debug)]
pub struct Plan<'a> {
    pub keep: &'a FileEntry,
    pub remove: Vec<&'a FileEntry>,
}

/// Decide which file survives in a group; everything else is scheduled for removal.
pub fn plan_group(group: &DuplicateGroup, strategy: KeepStrategy) -> Plan<'_> {
    // `group.files` is sorted by path, and `min_by_key` returns the first minimum,
    // so ties are always broken by path order.
    let keep_index = match strategy {
        KeepStrategy::First => 0,
        KeepStrategy::Oldest => index_of_min(&group.files, |f| f.modified),
        KeepStrategy::Newest => index_of_min(&group.files, |f| std::cmp::Reverse(f.modified)),
        KeepStrategy::ShortestPath => index_of_min(&group.files, |f| f.path.as_os_str().len()),
    };
    Plan {
        keep: &group.files[keep_index],
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
    use std::path::PathBuf;
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

    #[test]
    fn keep_strategies_pick_expected_file() {
        let g = group();
        let kept = |s| plan_group(&g, s).keep.path.to_str().unwrap().to_owned();
        assert_eq!(kept(KeepStrategy::First), "/a/long/path/copy.txt");
        assert_eq!(kept(KeepStrategy::Oldest), "/b/x.txt");
        assert_eq!(kept(KeepStrategy::Newest), "/c/deeper/y.txt");
        assert_eq!(kept(KeepStrategy::ShortestPath), "/b/x.txt");
    }

    #[test]
    fn plan_removes_all_but_one() {
        let g = group();
        let plan = plan_group(&g, KeepStrategy::Oldest);
        assert_eq!(plan.remove.len(), 2);
        assert!(plan.remove.iter().all(|f| f.path != plan.keep.path));
    }
}
