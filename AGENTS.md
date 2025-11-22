# Repository Guidelines

## Project Structure & Module Organization
The crate uses the standard Cargo layout. `src/main.rs` wires CLI subcommands to the implementations in `src/commands/`: `events.rs` (add/list/delete), `feeds.rs` (RSS), and `import.rs` (ICS ingest). `src/cli.rs` defines argument parsing with clap, `src/config.rs` loads `~/.config/toki-note/config.toml`, and `src/storage.rs` encapsulates SQLite access. Keep new features confined to focused modules, expose helpers via `pub(crate)` functions, and place static assets (e.g., `assets/logo.png`) or fixtures under `tests/fixtures/`.

## Build, Test, and Development Commands
- `cargo fmt` keeps rustfmt defaults enforced on every change.
- `cargo clippy --all-targets --all-features` must be clean; use `#[allow(...)] // rationale` sparingly.
- `cargo check` is the quickest sanity pass before committing.
- `cargo test` runs integration suites in `tests/` (currently driven by `assert_cmd`) plus unit tests; use `-- --nocapture` while debugging.
- `toki-note <subcommand>` exercises the built binary; `cargo run --bin toki-note -- <args>` is acceptable during development, but README examples prefer the installed `toki-note` command.

## Coding Style & Naming Conventions
Use Rust 2021+ idioms, `snake_case` functions, `CamelCase` types, and `SCREAMING_SNAKE_CASE` constants. Keep functions under ~40 lines, extract helpers in their own modules (declare via `mod foo;`). Order `use` statements by crate, document public APIs with `///`, and keep error messages actionable. Follow clap naming: long options like `--database` should also gain short flags only when justified in UX docs.

## Testing Guidelines
High-level behavior belongs in `tests/` with `assert_cmd::Command::cargo_bin` (or the `cargo_bin!` macro) to drive the CLI against temporary directories/DBs. Unit tests can stay near the code behind `#[cfg(test)]`. Name tests after behavior (`list_filters_by_day`) so `cargo test list` filtering is intuitive. When introducing new CLI output, capture and assert relevant lines to keep regressions visible in CI.

## Commit & Pull Request Guidelines
Commits are imperative and scoped, e.g., `Add list --tz flag` or `Refactor config parser`. Branch names can reflect the change (`feature/list-tz` or `bugfix/ical-offset`). Every PR description should contain `## Summary` (what changed) and `## Highlights` (why it matters). Link the relevant GitHub issue with `Closes #X`, note verification commands, and avoid bundling refactors with feature work unless the issue explicitly requests both.

## Security & Configuration Tips
SQLite files live under `$XDG_DATA_HOME/toki-note/toki-note.db` by default; never commit local DB snapshots. Validate user-provided paths (`--database`, `--output`, `--path`) before reading/writing, and prefer absolute paths for shared setups (e.g., Tailscale mounts). Config files may hold feed destinations, so keep them outside version control and document any sensitive defaults in PRs before merging.
