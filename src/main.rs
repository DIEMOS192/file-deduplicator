mod cli;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Scan(args) => {
            println!("scan: {:?}", args);
        }
        Command::Clean(args) => {
            println!("clean: {:?}", args);
        }
    }
    Ok(())
}
