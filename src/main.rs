mod cli;
mod scanner;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Scan(args) => {
            let result = scanner::scan(&args.options);
            for file in &result.files {
                println!("{} ({} bytes)", file.path.display(), file.size);
            }
            println!("{} files found", result.files.len());
        }
        Command::Clean(args) => {
            println!("clean: {:?}", args);
        }
    }
    Ok(())
}
