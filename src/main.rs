mod cleaner;
mod cli;
mod exclude;
mod finder;
mod hasher;
mod report;
mod scanner;

use std::io::{self, Write};

use anyhow::{bail, Result};
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

    for (i, group) in scan.groups.iter().enumerate() {
        let plan = cleaner::plan_group(group, args.keep);
        writeln!(
            out,
            "Group {} — {} files × {}",
            i + 1,
            group.files.len(),
            human_bytes(group.size)
        )?;
        writeln!(out, "  keep    {}", display_path(&plan.keep.path))?;

        // Never remove the copies if the one we're keeping has disappeared.
        if args.apply && !plan.keep.path.exists() {
            failures.push(format!(
                "{}: kept file no longer exists, skipping group",
                display_path(&plan.keep.path)
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
