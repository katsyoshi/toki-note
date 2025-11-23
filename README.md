# toki-note

<p align="center">
  <img src="assets/logo.png" alt="toki-note logo" width="200" />
</p>

`toki-note` is a small Rust CLI that stores personal schedules in a local SQLite database. It focuses on fast data entry via flags, ISO-8601 timestamps, tag support, and all-day events, making it convenient for terminal-driven workflows.

## Getting Started

```bash
rustup default stable   # first-time toolchain setup
cargo build             # compile the CLI
```

By default the binary writes to `$XDG_DATA_HOME/toki-note/toki-note.db` (e.g. `~/.local/share/toki-note/toki-note.db`). Override with `--database path/to/file.db` or set the `[database]` section described below.

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

You can also omit `--start` and provide `--date`/`--time` instead, with relative dates (e.g., `today`, `tomorrow`, `+2d`, `2日後`). When neither `--end` nor `--duration` is provided, a 30-minute slot is assumed.

```bash
toki-note add \
  --title "Daily standup" \
  --date tomorrow \
  --time 09:30 \
  --tag work
```

Add an all-day (or multi-day) entry by supplying dates instead of instants and the `--all-day` flag:

```bash
toki-note add --title "Vacation" --start 2025-08-10 --end 2025-08-15 --all-day --tag personal
```

All-day entries must use explicit `--end` (or omit it for a single day); `--duration` is ignored when `--all-day` is set.

Successful inserts print the assigned row id, which will later be used for listing or deleting records.

List all tracked events, ordered by start time (output uses your system timezone unless overridden with `--tz`). You can also use the `ls` alias:

```bash
toki-note list
# or
toki-note ls
```

Filter for a specific day (UTC boundary for the filter; display timezone may be overridden):

```bash
toki-note list --day 2025-08-10
```

Short flags are available, e.g. `toki-note list -d 2025-08-10 -z Europe/Paris` or `toki-note rss -o feed.xml`.

Force a specific timezone (use IANA names such as `Europe/Paris` or `America/New_York`):

```bash
toki-note list --tz Europe/Paris
```

Delete an event by id (see ids from `list` output) or by title:

```bash
toki-note delete --id 42
toki-note delete --title "1:1 sync"
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

Import events from an iCalendar file (duplicates are skipped by UID):

```bash
toki-note import --path path/to/events.ics
```

### Sharing a database over Tailscale

If you have multiple machines connected via Tailscale (or another VPN) and want to share the same SQLite database, you can:

1. Expose a shared directory on one machine (NFS/Samba/SSHFS/etc.) over the VPN
2. Mount that directory on other machines
3. Point `toki-note` to the shared file, e.g. `toki-note --database /mnt/toki-note/toki-note.db`

SQLite is not designed for concurrent writers over a network filesystem, so try to avoid simultaneous writes. This setup is best when only one machine edits at a time (read-only access from others is fine).

## Configuration

Optional settings live in `$XDG_CONFIG_HOME/toki-note/config.toml` (e.g. `~/.config/toki-note/config.toml`). You can predefine paths for the database and feed/import outputs:

```toml
[database]
path = "/path/to/custom.db"

[rss]
output = "/path/to/feed.xml"

[ical]
output = "/path/to/feed.ics"

[import]
source = "/path/to/events.ics"
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
