# toki-note

`toki-note` is a small Rust CLI that stores personal schedules in a local SQLite database. It focuses on fast data entry via flags, ISO-8601 timestamps, tag support, and all-day events, making it convenient for terminal-driven workflows.

## Getting Started

```bash
rustup default stable   # first-time toolchain setup
cargo build             # compile the CLI
```

By default the binary writes to `$XDG_DATA_HOME/toki-note/toki-note.db` (e.g. `~/.local/share/toki-note/toki-note.db`). Override with `--database path/to/file.db` or set the `database` field in the config file described below.

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

List all tracked events, ordered by start time (output uses your system timezone unless overridden with `--tz`):

```bash
toki-note list
```

Filter for a specific day (UTC boundary for the filter; display timezone may be overridden):

```bash
toki-note list --day 2025-08-10
```

Force a specific timezone (use IANA names such as `Europe/Paris` or `America/New_York`):

```bash
toki-note list --tz Europe/Paris
```

Generate an RSS feed (stdout) and redirect to a file:

```bash
toki-note rss --title "Private schedule" --link https://example.com --tz Asia/Tokyo > schedule.xml
```

You can combine `--day` and `--tz` to emit limited feeds (e.g., `toki-note rss --day 2025-08-10 --tz Europe/Paris`).

Use `--output` to write the feed directly:

```bash
toki-note rss --tz Asia/Tokyo --output ~/.cache/toki-note/feed.xml
```

Generate an iCalendar file:

```bash
toki-note ical --day 2025-08-10 --tz America/Los_Angeles --output schedule.ics
```

## Configuration

Optional settings live in `$XDG_CONFIG_HOME/toki-note/config.toml` (e.g. `~/.config/toki-note/config.toml`). You can predefine paths for the database and feed outputs:

```toml
database = "/path/to/custom.db"
rss_output = "/path/to/feed.xml"
ical_output = "/path/to/feed.ics"
```

This file is read on startup before CLI flags are processed; flags always win over config values.

## Development

- `cargo fmt` to keep Rust style consistent.
- `cargo clippy --all-targets --all-features` to lint and refuse regressions.
- `cargo check` for fast feedback; `cargo test` once querying/listing commands land.

The SQLite schema is created automatically on first run and consists of `events` and `event_tags`. Each transaction writes the event first, then lowercases all tags before storing them to avoid duplicates. Extend the CLI by adding more `Subcommand` variants in `src/main.rs`. Keep DB migrations backward compatible for existing `.db` files.

## Contributors

- katsyoshi
- Codex (AI assistant)
