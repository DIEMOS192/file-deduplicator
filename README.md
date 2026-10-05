# dedup — file deduplicator

A small, fast, **safe-by-default** command-line tool for finding and removing duplicate files. Written in Rust.

## Usage

```sh
# Find duplicates (read-only)
dedup scan ~/Pictures ~/Downloads

# Ignore files under 1 MB, output JSON
dedup scan ~/Downloads --min-size 1000000 --json

# Skip extra folders/files (repeatable; node_modules, .git, target, ... are skipped by default)
dedup scan ~/Projects --exclude '*.tmp' --exclude '**/Photos/raw/**'

# Preview what would be removed (dry run — nothing is touched)
dedup clean ~/Downloads --keep oldest

# Actually move the extra copies to the recycle bin
dedup clean ~/Downloads --keep oldest --apply

# Permanently delete instead of using the recycle bin
dedup clean ~/Downloads --action delete --apply

# Big scans: compare against everything, but only ever remove copies in Downloads
dedup clean ~/Pictures ~/Documents ~/Downloads --only-delete-in ~/Downloads
```

| Flag | Meaning |
|------|---------|
| `--min-size <BYTES>` | skip files smaller than this (default `1`, so empty files are skipped) |
| `--hidden` | include hidden files and directories |
| `--follow-links` | follow symbolic links |
| `--exclude <GLOB>` | skip matching files/folders; repeatable. No `/` matches a name anywhere (`node_modules`, `*.tmp`), with `/` matches the full path (`**/Photos/raw/**`) |
| `--no-default-excludes` | also scan `node_modules`, `.git`, `target`, `.venv`, `$RECYCLE.BIN` and the other built-in excludes |
| `--include-online-only` | also read cloud placeholders (OneDrive "online-only" files). Skipped by default because reading them downloads them |
| `--json` | machine-readable output (`scan` only) |
| `--keep <oldest\|newest\|shortest-path\|first>` | which copy survives in each group (`clean` only) |
| `--action <trash\|delete>` | how to remove the others (`clean` only, default `trash`) |
| `--only-delete-in <DIR>` | only remove copies inside this folder; copies elsewhere are always kept (`clean` only, repeatable) |
| `--apply` | perform the removal; without it `clean` is a dry run |

Run `dedup --help` or `dedup <command> --help` for details.

### Cleaning large folders safely

Identical bytes don't always mean a copy is unneeded: programs, projects and backups often keep identical files on purpose. When scanning a broad folder:

- Prefer folders that hold your own files (Downloads, Pictures, Documents) over a whole profile.
- Use `--min-size` to focus on the files that actually take space.
- Use `--only-delete-in` so removals are confined to a folder you consider disposable.
- Keep the default `--action trash` so anything can be restored.

## How it works

Files are grouped by size, then by a hash of their first 16 KiB, then by a full BLAKE3 hash. Only files that still match at each stage get read further, so most files are never fully read. See [PLAN.md](PLAN.md) for the design and roadmap.

## Building from source

### 1. Install Rust

**Windows**
1. Install the [Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) ("Desktop development with C++"). Rust uses its linker.
2. Install rustup: `winget install Rustlang.Rustup`, or download `rustup-init.exe` from <https://rustup.rs>.
3. Open a **new** terminal so `cargo` is on your `PATH`.

**Linux / macOS**
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Check the install with `cargo --version`.

### 2. Build and run

```sh
cargo build --release        # binary at target/release/dedup(.exe)
cargo run -- scan .          # build + run in one step (args after --)
cargo install --path .       # install `dedup` to ~/.cargo/bin
```

### 3. Develop

```sh
cargo test                   # run all tests
cargo fmt                    # format code
cargo clippy                 # lint
```

## License

MIT
