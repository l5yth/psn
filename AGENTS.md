# Repository Guidelines

## Project Structure & Module Organization
- `src/main.rs`: current application entry point and core TUI/process logic.
- `.github/workflows/`: CI pipelines for Rust quality checks and Linux packaging validation.
- `Cargo.toml`: crate metadata and dependencies.
- `target/`: local build artifacts (generated; do not commit).

As the codebase grows, prefer splitting logic from `main.rs` into focused modules under `src/` (for example `src/ui.rs`, `src/process.rs`) and add integration tests under `tests/`.

## Build, Test, and Development Commands
- `cargo check --all --all-features`: fast compile validation.
- `cargo run --release --`: run the TUI locally with release optimizations.
- `cargo test --all --all-features --verbose`: execute test suite.
- `cargo fmt --all`: apply standard Rust formatting.
- `cargo clippy --all-targets --all-features -- -D warnings`: lint and fail on warnings.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items`: build docs with warnings denied.

Use these same commands before opening a PR; they mirror CI in `.github/workflows/rust.yml`.

## Coding Style & Naming Conventions
- Follow `rustfmt` defaults (4-space indentation; no tabs).
- Keep functions and variables in `snake_case`, types/traits in `CamelCase`, constants in `SCREAMING_SNAKE_CASE`.
- Prefer small, single-purpose functions and explicit error propagation with `anyhow::Result`.
- Keep terminal/UI strings concise and user-facing messages actionable.

## Testing Guidelines
- Use Rust’s built-in test framework (`#[test]`, `cargo test`).
- Place unit tests close to implementation (`mod tests` in source files) and integration tests in `tests/`.
- Name tests to describe behavior, e.g. `refresh_rows_sorts_by_cpu_then_mem`.
- Coverage is collected in CI via `cargo llvm-cov`; add tests for new logic and bug fixes.

## Commit & Pull Request Guidelines
- Current branch has no commit history yet; adopt Conventional Commits going forward (e.g. `feat: add process filter`, `fix: handle missing uid`).
- Keep commits focused and buildable.
- PRs should include a clear summary and motivation, linked issue (if any), test evidence (command output or checklist), and screenshots/GIFs for TUI-visible behavior changes.
