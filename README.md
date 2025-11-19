# toki-note

`toki-note` is a small Rust CLI that stores personal schedules in a local SQLite database. It focuses on fast data entry via flags, ISO-8601 timestamps, tag support, and all-day events, making it convenient for terminal-driven workflows.

## Getting Started

```bash
rustup default stable   # first-time toolchain setup
cargo build             # compile the CLI
```

The binary lookups `toki-note.db` in the current directory by default. Override with `--database path/to/file.db` when needed.

## Usage

Add a timed event:

```bash
toki-note add \
  --title "1:1 sync" \
  --start "2025-02-01T10:00:00+09:00" \
  --duration 30m \
  --note "Zoom 1234" \
  --tag work --tag sync
```

`--end` still works for absolute end instants and always overrides `--duration` when both are supplied.

Add an all-day (or multi-day) entry by supplying dates instead of instants and the `--all-day` flag:

```bash
toki-note add --title "Vacation" --start 2025-08-10 --end 2025-08-15 --all-day --tag personal
```

All-day entries must use explicit `--end` (or omit it for a single day); `--duration` is ignored when `--all-day` is set.

Successful inserts print the assigned row id, which will later be used for listing or deleting records.

## Development

- `cargo fmt` to keep Rust style consistent.
- `cargo clippy --all-targets --all-features` to lint and refuse regressions.
- `cargo check` for fast feedback; `cargo test` once querying/listing commands land.

The SQLite schema is created automatically on first run and consists of `events` and `event_tags`. Each transaction writes the event first, then lowercases all tags before storing them to avoid duplicates. Extend the CLI by adding more `Subcommand` variants in `src/main.rs`. Keep DB migrations backward compatible for existing `.db` files.*** End Patch
