# Repository Guidelines

## Project Structure & Module Organization
The crate follows standard Cargo layout: `src/main.rs` hosts the application entry point and should orchestrate any future modules in `src/`. Keep DTOs, services, and CLI adapters in separate submodules for clarity. Workspace metadata lives in `Cargo.toml`; add fixtures or assets under `tests/fixtures/` if needed to avoid polluting `src/`.

## Build, Test, and Development Commands
- `cargo check` — fast validation of type correctness; run before committing structural changes.
- `cargo clippy --all-targets --all-features` — lints with the default profile; fix warnings or whitelist with `#[allow]` plus a short comment.
- `cargo fmt` — enforce rustfmt defaults; configure via `rustfmt.toml` only if absolutely necessary.
- `cargo test` — executes unit, doc, and integration tests; pass `-- --nocapture` when debugging output.
- `cargo run` — runs the CLI locally; supply example args via `cargo run -- <flags>`.

## Coding Style & Naming Conventions
Adopt Rust 2024 idioms: prefer explicit modules, use `snake_case` for functions, `CamelCase` for types, and `SCREAMING_SNAKE_CASE` for constants. Keep functions under ~40 lines; factor helpers into `mod foo;` files when logic grows. Document public APIs with `///` comments, and gate experimental code behind `#[cfg(feature = "...")]`. Use `rustfmt` defaults (4‑space indent, trailing commas on multiline enums) and keep imports sorted by crate.

## Testing Guidelines
Add unit tests near the implementation inside `#[cfg(test)] mod tests`. Integration tests should live under `tests/` and mirror feature areas (e.g., `tests/cli.rs`). Name tests after behavior, `test_parses_markdown_metadata`, to ease filtering via `cargo test parse`. Target meaningful coverage for new modules; include doc tests for example snippets in README fragments.

## Commit & Pull Request Guidelines
Commits should be scoped and imperative: `Add parser for front-matter`, `Fix panic on empty note`. Squash noisy WIP commits before pushing. Pull requests must describe the motivation, summarize implementation, and list verification steps (commands run, screenshots when CLI output changes). Cross-reference issues with `Closes #123`. Keep PRs self-contained, avoid unrelated formatting, and ensure CI passes `check`, `fmt`, `clippy`, and `test` stages before requesting review.

Each pull request description should include:
- `## Summary` explaining the main changes
- `## Highlights` describing why the change matters or key benefits

## Security & Configuration Tips
Never commit secrets or local `.env` files; prefer environment variables injected at runtime. Validate and sanitize all file paths before reading user content. When adding dependencies, ensure they are actively maintained and avoid enabling default features you do not use to minimize the attack surface.
