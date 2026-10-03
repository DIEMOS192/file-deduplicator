use std::io::{self, Write};

use serde::Serialize;

use crate::finder::DuplicateGroup;
use crate::hasher;

#[derive(Serialize)]
struct JsonReport<'a> {
    files_scanned: usize,
    duplicate_groups: usize,
    duplicate_files: usize,
    wasted_bytes: u64,
    groups: Vec<JsonGroup>,
    errors: &'a [String],
}

#[derive(Serialize)]
struct JsonGroup {
    size: u64,
    hash: String,
    files: Vec<String>,
}

pub struct Summary {
    pub files_scanned: usize,
    pub groups: usize,
    /// Files that are copies beyond the first in each group.
    pub duplicate_files: usize,
    pub wasted_bytes: u64,
}

pub fn summarize(files_scanned: usize, groups: &[DuplicateGroup]) -> Summary {
    Summary {
        files_scanned,
        groups: groups.len(),
        duplicate_files: groups.iter().map(|g| g.files.len() - 1).sum(),
        wasted_bytes: groups.iter().map(DuplicateGroup::wasted_bytes).sum(),
    }
}

/// Print duplicate groups in a human-friendly layout.
pub fn print_human(
    out: &mut impl Write,
    files_scanned: usize,
    groups: &[DuplicateGroup],
    errors: &[String],
) -> io::Result<()> {
    for (i, group) in groups.iter().enumerate() {
        writeln!(
            out,
            "Group {} — {} files × {}",
            i + 1,
            group.files.len(),
            human_bytes(group.size)
        )?;
        for file in &group.files {
            writeln!(out, "  {}", display_path(&file.path))?;
        }
        writeln!(out)?;
    }
    print_errors(out, errors)?;
    print_summary(out, &summarize(files_scanned, groups))
}

pub fn print_summary(out: &mut impl Write, s: &Summary) -> io::Result<()> {
    if s.groups == 0 {
        return writeln!(out, "No duplicates found among {} files.", s.files_scanned);
    }
    writeln!(
        out,
        "Scanned {} files: {} duplicate groups, {} redundant copies, {} reclaimable.",
        s.files_scanned,
        s.groups,
        s.duplicate_files,
        human_bytes(s.wasted_bytes)
    )
}

pub fn print_errors(out: &mut impl Write, errors: &[String]) -> io::Result<()> {
    if errors.is_empty() {
        return Ok(());
    }
    writeln!(out, "{} files could not be read:", errors.len())?;
    for e in errors {
        writeln!(out, "  {e}")?;
    }
    writeln!(out)
}

/// Print duplicate groups as a single JSON document.
pub fn print_json(
    out: &mut impl Write,
    files_scanned: usize,
    groups: &[DuplicateGroup],
    errors: &[String],
) -> io::Result<()> {
    let s = summarize(files_scanned, groups);
    let report = JsonReport {
        files_scanned,
        duplicate_groups: s.groups,
        duplicate_files: s.duplicate_files,
        wasted_bytes: s.wasted_bytes,
        groups: groups
            .iter()
            .map(|g| JsonGroup {
                size: g.size,
                hash: hasher::to_hex(&g.hash),
                files: g.files.iter().map(|f| display_path(&f.path)).collect(),
            })
            .collect(),
        errors,
    };
    serde_json::to_writer_pretty(&mut *out, &report)?;
    writeln!(out)
}

/// Strip the `\\?\` prefix that `canonicalize` adds on Windows, for readability.
pub fn display_path(path: &std::path::Path) -> String {
    let s = path.display().to_string();
    s.strip_prefix(r"\\?\").map(str::to_owned).unwrap_or(s)
}

/// Format a byte count like `1.5 MiB`.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_bytes_formats_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1536), "1.5 KiB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MiB");
    }
}
