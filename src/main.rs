mod cli;
mod finder;
mod hasher;
mod report;
mod scanner;

use std::io::{self, Write};

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command, ScanOptions};
use finder::DuplicateGroup;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Scan(args) => run_scan(&args.options),
        Command::Clean(args) => {
            println!("clean: {:?}", args);
            Ok(())
        }
    }
}

/// Everything learned from scanning: the duplicate groups plus bookkeeping.
struct Scan {
    files_scanned: usize,
    groups: Vec<DuplicateGroup>,
    errors: Vec<String>,
}

fn find(options: &ScanOptions) -> Scan {
    let scanned = scanner::scan(options);
    let files_scanned = scanned.files.len();
    let found = finder::find_duplicates(scanned.files);
    let mut errors = scanned.errors;
    errors.extend(found.errors);
    Scan {
        files_scanned,
        groups: found.groups,
        errors,
    }
}

fn run_scan(options: &ScanOptions) -> Result<()> {
    let scan = find(options);
    let mut out = io::stdout().lock();
    if options.json {
        report::print_json(&mut out, scan.files_scanned, &scan.groups, &scan.errors)?;
    } else {
        report::print_human(&mut out, scan.files_scanned, &scan.groups, &scan.errors)?;
    }
    out.flush()?;
    Ok(())
}
