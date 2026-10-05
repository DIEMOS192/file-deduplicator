//! End-to-end tests that run the compiled `dedup` binary.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

fn dedup() -> Command {
    Command::cargo_bin("dedup").unwrap()
}

/// Creates: two copies of "alpha", three copies of "bravo!", and one unique file.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir(root.join("sub")).unwrap();
    for (name, data) in [
        ("a1.txt", "alpha"),
        ("sub/a2.txt", "alpha"),
        ("b1.txt", "bravo!"),
        ("b2.txt", "bravo!"),
        ("sub/b3.txt", "bravo!"),
        ("unique.txt", "charlie"),
    ] {
        fs::write(root.join(name), data).unwrap();
    }
    dir
}

fn count_files(dir: &Path) -> usize {
    walk(dir).len()
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[test]
fn scan_reports_duplicate_groups() {
    let dir = fixture();
    dedup()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("2 duplicate groups"))
        .stdout(predicate::str::contains("3 redundant copies"));
}

#[test]
fn scan_json_is_valid() {
    let dir = fixture();
    let output = dedup()
        .args(["scan", "--json"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["files_scanned"], 6);
    assert_eq!(json["duplicate_groups"], 2);
    assert_eq!(json["wasted_bytes"], 5 + 2 * 6);
}

#[test]
fn scan_with_no_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("only.txt"), "x").unwrap();
    dedup()
        .arg("scan")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No duplicates found"));
}

#[test]
fn clean_is_a_dry_run_by_default() {
    let dir = fixture();
    dedup()
        .arg("clean")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));
    assert_eq!(count_files(dir.path()), 6, "nothing should be removed");
}

#[test]
fn clean_apply_delete_keeps_one_of_each() {
    let dir = fixture();
    dedup()
        .args(["clean", "--keep", "first", "--action", "delete", "--apply"])
        .arg(dir.path())
        .assert()
        .success();
    assert_eq!(count_files(dir.path()), 3);
    // `first` keeps the alphabetically first path in each group.
    assert!(dir.path().join("a1.txt").exists());
    assert!(dir.path().join("b1.txt").exists());
    assert!(dir.path().join("unique.txt").exists());
}

#[test]
fn missing_path_argument_is_an_error() {
    dedup().arg("scan").assert().failure();
}

#[test]
fn exclude_and_default_excludes() {
    let dir = fixture();
    let nm = dir.path().join("node_modules");
    fs::create_dir(&nm).unwrap();
    fs::write(nm.join("a3.txt"), "alpha").unwrap();

    // node_modules is skipped by default, and `sub` is excluded explicitly,
    // leaving only the b1/b2 pair.
    dedup()
        .args(["scan", "--exclude", "sub"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Scanned 4 files: 1 duplicate groups",
        ));

    dedup()
        .args(["scan", "--no-default-excludes"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Scanned 7 files: 2 duplicate groups",
        ));
}

#[test]
fn invalid_exclude_pattern_fails() {
    let dir = fixture();
    dedup()
        .args(["scan", "--exclude", "["])
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid --exclude pattern"));
}

#[test]
fn only_delete_in_never_touches_files_outside() {
    let dir = fixture();
    dedup()
        .args(["clean", "--action", "delete", "--apply", "--only-delete-in"])
        .arg(dir.path().join("sub"))
        .arg(dir.path())
        .assert()
        .success();
    // Both copies inside `sub` are gone; every copy outside it survives,
    // including the b1/b2 pair that duplicates each other.
    assert!(!dir.path().join("sub").join("a2.txt").exists());
    assert!(!dir.path().join("sub").join("b3.txt").exists());
    for name in ["a1.txt", "b1.txt", "b2.txt", "unique.txt"] {
        assert!(dir.path().join(name).exists(), "{name} should be kept");
    }
}

#[test]
fn only_delete_in_reports_untouched_groups() {
    let dir = fixture();
    fs::create_dir(dir.path().join("empty")).unwrap();
    dedup()
        .args(["clean", "--only-delete-in"])
        .arg(dir.path().join("empty"))
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("2 groups left untouched"))
        .stdout(predicate::str::contains("would move to trash 0 files"));
}

#[test]
fn only_delete_in_missing_dir_fails() {
    let dir = fixture();
    dedup()
        .args(["clean", "--only-delete-in"])
        .arg(dir.path().join("nope"))
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("--only-delete-in"));
    assert_eq!(count_files(dir.path()), 6);
}
