mod cli;
mod finder;
mod hasher;
mod scanner;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Scan(args) => {
            let scanned = scanner::scan(&args.options);
            let total = scanned.files.len();
            let found = finder::find_duplicates(scanned.files);
            for group in &found.groups {
                println!("{} ({} bytes)", hasher::to_hex(&group.hash), group.size);
                for file in &group.files {
                    println!("  {}", file.path.display());
                }
            }
            println!(
                "{} files scanned, {} duplicate groups",
                total,
                found.groups.len()
            );
        }
        Command::Clean(args) => {
            println!("clean: {:?}", args);
        }
    }
    Ok(())
}
