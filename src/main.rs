mod cleaner;
mod cli;
mod exclude;
mod finder;
mod hasher;
mod report;
mod scanner;

use std::io::{self, Write};

use anyhow::{bail, Context, Result};
use clap::Parser;

use cli::{Action, CleanArgs, Cli, Command, ScanArgs, ScanOptions};
use finder::DuplicateGroup;
use report::{display_path, human_bytes};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Scan(args) => run_scan(&args),
        Command::Clean(args) => run_clean(&args),
    }
}

/// Everything learned from scanning: the duplicate groups plus bookkeeping.
struct Scan {
    files_scanned: usize,
    groups: Vec<DuplicateGroup>,
    errors: Vec<String>,
}

fn find(options: &ScanOptions) -> Result<Scan> {
    let scanned = scanner::scan(options)?;
    if scanned.online_only_skipped > 0 {
        eprintln!(
            "note: skipped {} online-only cloud files (use --include-online-only to read them)",
            scanned.online_only_skipped
        );
    }
    let files_scanned = scanned.files.len();
    let found = finder::find_duplicates(scanned.files);
    let mut errors = scanned.errors;
    errors.extend(found.errors);
    Ok(Scan {
        files_scanned,
        groups: found.groups,
        errors,
    })
}

fn run_scan(args: &ScanArgs) -> Result<()> {
    let scan = find(&args.options)?;
    let mut out = io::stdout().lock();
    if args.json {
        report::print_json(&mut out, scan.files_scanned, &scan.groups, &scan.errors)?;
    } else {
        report::print_human(&mut out, scan.files_scanned, &scan.groups, &scan.errors)?;
    }
    out.flush()?;
    Ok(())
}

fn run_clean(args: &CleanArgs) -> Result<()> {
    let deletable_dirs = args
        .only_delete_in
        .iter()
        .map(|d| {
            std::fs::canonicalize(d)
                .with_context(|| format!("--only-delete-in {}: not found", d.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let scan = find(&args.options)?;
    let mut out = io::stdout().lock();
    report::print_errors(&mut out, &scan.errors)?;

    if scan.groups.is_empty() {
        writeln!(
            out,
            "No duplicates found among {} files.",
            scan.files_scanned
        )?;
        return Ok(());
    }

    let verb = match args.action {
        Action::Trash => "move to trash",
        Action::Delete => "delete",
    };
    let mut removed = 0usize;
    let mut freed = 0u64;
    let mut failures = Vec::new();
    let mut shown = 0usize;
    let mut untouched = 0usize;

    for group in &scan.groups {
        let plan = cleaner::plan_group(group, args.keep, &deletable_dirs);
        if plan.remove.is_empty() {
            untouched += 1;
            continue;
        }
        shown += 1;
        writeln!(
            out,
            "Group {} — {} files × {}",
            shown,
            group.files.len(),
            human_bytes(group.size)
        )?;
        for file in &plan.keep {
            writeln!(out, "  keep    {}", display_path(&file.path))?;
        }

        // Never remove the copies if a file we're keeping has disappeared.
        if let Some(missing) = plan.keep.iter().find(|f| args.apply && !f.path.exists()) {
            failures.push(format!(
                "{}: kept file no longer exists, skipping group",
                display_path(&missing.path)
            ));
            continue;
        }

        for file in &plan.remove {
            if args.apply {
                match cleaner::remove(&file.path, args.action) {
                    Ok(()) => {
                        removed += 1;
                        freed += file.size;
                        writeln!(out, "  removed {}", display_path(&file.path))?;
                    }
                    Err(e) => failures.push(format!("{}: {}", display_path(&file.path), e)),
                }
            } else {
                removed += 1;
                freed += file.size;
                writeln!(out, "  {:<7} {}", "remove", display_path(&file.path))?;
            }
        }
        writeln!(out)?;
    }

    if untouched > 0 {
        writeln!(
            out,
            "{} groups left untouched: no copies inside --only-delete-in.",
            untouched
        )?;
    }
    if args.apply {
        writeln!(
            out,
            "Done: {} {} files, freed {}.",
            past_tense(args.action),
            removed,
            human_bytes(freed)
        )?;
    } else {
        writeln!(
            out,
            "Dry run: would {} {} files, freeing {}. Re-run with --apply to do it.",
            verb,
            removed,
            human_bytes(freed)
        )?;
    }
    out.flush()?;

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("error: {f}");
        }
        bail!("{} files could not be removed", failures.len());
    }
    Ok(())
}

fn past_tense(action: Action) -> &'static str {
    match action {
        Action::Trash => "trashed",
        Action::Delete => "deleted",
    }
}
