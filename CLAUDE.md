# Repository Guidelines

## Purpose
`psn` is a Linux-first Rust TUI for process status navigation and signal-based process control (for example sending signals 1-9 to the selected process).

## Project Structure & Module Organization
- `src/main.rs`: thin binary entry point only (argument parsing, wiring, startup/shutdown).
- `src/lib.rs`: crate root; re-exports public API for integration tests.
- `src/cli.rs`: command-line argument parsing (`clap`).
- `src/app.rs`: mutable application state (selection, filter, collapsed pids).
- `src/runtime.rs`: key mapping, action dispatch, event loop, terminal setup/restore.
- `src/ui.rs`: rendering only (ratatui widgets).
- `src/process.rs`: process discovery, filter compilation, sort mapping.
- `src/signal.rs`: signal mapping and send helpers.
- `src/model.rs`: shared data types (`ProcRow`).
- `src/tree.rs`: tree display order and collapse logic.
- `src/debug_tui.rs`: deterministic debug/demo mode (no real processes).
- `tests/`: integration tests.
- `.github/workflows/`: CI for formatting, linting, tests, docs, and coverage.
- `Cargo.toml`: crate metadata/dependencies.
- `packaging/`: distro packaging files (Arch/Gentoo).
- `target/`: generated build artifacts; never commit.

Future refactor: extract `src/terminal.rs` for terminal setup/restore lifecycle (currently in `runtime.rs`).

Module rules:
- Keep modules small and cohesive.
- Avoid cross-module cycles and hidden shared mutable state.
- Prefer pure functions for business logic; isolate side effects at boundaries.

## Build, Test, and Development Commands
- `cargo check --all --all-features`
- `cargo run --release --`
- `cargo test --all --all-features --verbose`
- `cargo fmt --all`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items`
- `cargo llvm-cov --workspace --lcov --output-path lcov.info --fail-under-lines 100`

Before every PR, run the full quality gate locally: fmt-check, clippy, tests, docs, and coverage at 100% lines.

## Coding Style & Naming Conventions
- Follow `rustfmt` defaults (4 spaces, no tabs).
- Naming: `snake_case` (functions/vars), `CamelCase` (types/traits), `SCREAMING_SNAKE_CASE` (constants).
- Prefer small, single-purpose functions with explicit `anyhow::Result` error propagation where appropriate.
- Keep code minimalist: no dead code, no speculative abstractions, no unused dependencies.
- Keep UI text concise, actionable, and consistent with terminal constraints.

## Documentation Requirements
- Public and non-trivial internal APIs must have inline Rust doc comments explaining behavior and invariants.
- Document why non-obvious decisions exist (sorting rules, fallback behavior, signal mapping limits, etc.).
- Keep docs close to code and update them in the same change as logic updates.
- `cargo doc` must pass with warnings denied.

## Testing Requirements
- Use Rust’s built-in framework (`#[test]`) for unit tests and `tests/` for integration tests.
- Every non-trivial function must have unit tests covering normal, edge, and error behavior.
- Prefer deterministic tests for sorting/filtering/signal mapping logic.
- Coverage target is strict: 100% line coverage for project code.
- If coverage is below 100%, add tests in the same PR until the gate passes.
- Test names must describe behavior (example: `refresh_rows_filters_by_name_or_cmd_case_insensitive`).

## Commit & Pull Request Guidelines
- Use Conventional Commits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`).
- Keep commits focused, buildable, and minimal.
- PRs must include a clear summary and motivation.
- PRs must include linked issue(s), when applicable.
- PRs must include test evidence (commands run and results).
- PRs must include screenshots/GIFs for user-visible TUI changes.
- PRs must explicitly call out module boundaries touched and architectural impacts.
