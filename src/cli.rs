use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Find and safely remove duplicate files.
#[derive(Debug, Parser)]
#[command(name = "dedup", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List groups of duplicate files (read-only).
    Scan(ScanArgs),
    /// Remove duplicates, keeping one file per group. Dry run unless --apply is given.
    Clean(CleanArgs),
}

/// Options shared by every command that scans directories.
#[derive(Debug, Args)]
pub struct ScanOptions {
    /// Directories (or files) to scan.
    #[arg(required = true, value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Ignore files smaller than this many bytes.
    #[arg(long, default_value_t = 1, value_name = "BYTES")]
    pub min_size: u64,

    /// Include hidden files and directories.
    #[arg(long)]
    pub hidden: bool,

    /// Follow symbolic links.
    #[arg(long)]
    pub follow_links: bool,

    /// Skip files and directories matching this glob (repeatable).
    ///
    /// Patterns without '/' match a name anywhere, e.g. `node_modules` or `*.tmp`.
    /// Patterns with '/' match the full path, e.g. `**/Photos/raw/**`.
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Don't skip the built-in excludes: node_modules, bower_components, .git, .svn,
    /// .hg, target, __pycache__, .venv, venv, .tox, $RECYCLE.BIN, System Volume Information.
    #[arg(long)]
    pub no_default_excludes: bool,
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    #[command(flatten)]
    pub options: ScanOptions,

    /// Print results as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct CleanArgs {
    #[command(flatten)]
    pub options: ScanOptions,

    /// Which file to keep in each duplicate group.
    #[arg(long, value_enum, default_value_t = KeepStrategy::Oldest)]
    pub keep: KeepStrategy,

    /// How to remove the other files.
    #[arg(long, value_enum, default_value_t = Action::Trash)]
    pub action: Action,

    /// Actually remove files. Without this flag nothing is changed.
    #[arg(long)]
    pub apply: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum KeepStrategy {
    /// Keep the file with the oldest modification time.
    Oldest,
    /// Keep the file with the newest modification time.
    Newest,
    /// Keep the file with the shortest path.
    ShortestPath,
    /// Keep the first file in sorted path order.
    First,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Action {
    /// Move to the operating system's recycle bin / trash.
    Trash,
    /// Permanently delete.
    Delete,
}
