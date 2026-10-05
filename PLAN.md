# Project Plan — `dedup`

A small, safe, fast command-line tool that finds duplicate files and helps you clean them up.

## Goals

- **Simple interface**: two main commands, sensible defaults.
- **Safe by default**: never deletes anything unless you explicitly say so (`--apply`), and prefers the recycle bin over permanent deletion.
- **Fast**: avoids hashing files that cannot be duplicates, hashes in parallel.
- **Cross-platform**: Windows, Linux, macOS.

## Command design

```text
dedup scan <PATH>...                 # list duplicate groups (read-only)
    --min-size <BYTES>               # ignore small files (default: 1)
    --hidden                         # include hidden files/dirs
    --follow-links                   # follow symbolic links
    --exclude <GLOB>                 # skip matching names/paths (repeatable)
    --no-default-excludes            # don't skip node_modules, .git, target, ...
    --json                           # machine-readable output

dedup clean <PATH>...                # remove duplicates, keeping one per group
    --keep <oldest|newest|shortest-path|first>   (default: oldest)
    --action <trash|delete>          # trash = recycle bin (default)
    --apply                          # actually do it; without this it's a dry run
    (+ same filters as scan)
```

## How duplicate detection works

The pipeline narrows candidates in stages so most files are never fully read:

1. **Walk** the given directories and collect regular files (`walkdir`).
2. **Group by size**. A file with a unique size cannot have a duplicate.
3. **Partial hash**: hash the first 16 KiB of each remaining candidate (`blake3`).
4. **Full hash**: hash entire contents of files whose partial hashes collide.
5. Files with identical full hashes form a **duplicate group**.

Stages 3 and 4 run in parallel across CPU cores (`rayon`).

## Project structure

```text
file-deduplicator/
├── Cargo.toml          # package manifest + dependencies
├── src/
│   ├── main.rs         # entry point: parse args, dispatch commands
│   ├── cli.rs          # clap command/flag definitions
│   ├── scanner.rs      # directory walking + filtering
│   ├── exclude.rs      # --exclude glob matching + default excludes
│   ├── hasher.rs       # partial/full hashing helpers
│   ├── finder.rs       # size → partial → full grouping pipeline
│   ├── report.rs       # human-readable and JSON output
│   └── cleaner.rs      # choosing the keeper, trash/delete actions
├── tests/
│   └── cli.rs          # end-to-end tests running the real binary
└── .github/workflows/
    └── ci.yml          # fmt + clippy + tests on Windows/Linux/macOS
```

## Dependencies

| Crate | Why |
|-------|-----|
| `clap` | argument parsing with derive macros |
| `walkdir` | recursive directory traversal |
| `globset` | `--exclude` glob patterns |
| `blake3` | very fast cryptographic hash |
| `rayon` | easy data parallelism |
| `anyhow` | ergonomic error handling |
| `serde`, `serde_json` | `--json` output |
| `trash` | move files to the OS recycle bin |
| `assert_cmd`, `predicates`, `tempfile` | (dev) integration tests |

## Milestones

Each milestone is its own commit, pushed to GitHub.

- [x] **M0 — Setup**: Rust toolchain, repo, plan, `.gitignore`, Cargo project.
- [x] **M1 — CLI skeleton**: `scan` / `clean` subcommands parse, `--help` works.
- [x] **M2 — Scanner**: walk directories, filters (min size, hidden, symlinks).
- [x] **M3 — Duplicate finder**: size → partial hash → full hash pipeline + unit tests.
- [x] **M4 — Reporting**: human-readable summary and `--json` output.
- [x] **M5 — Cleaning**: keep strategies, trash/delete, dry run by default.
- [x] **M6 — Tests & CI**: integration tests, GitHub Actions.
- [x] **M7 — Excludes**: `--exclude` globs plus built-in excludes for dependency/build/VCS folders.

## Ideas for later

- `hardlink` action (replace duplicates with hard links to save space without losing paths)
- Interactive mode (choose which file to keep per group)
- Hash cache to make re-scans of large trees instant
- Progress bar for large scans (`indicatif`)
