<!-- Copyright (c) 2026 l5yth -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# psn

[![Rust](https://github.com/l5yth/psn/actions/workflows/rust.yml/badge.svg)](https://github.com/l5yth/psn/actions/workflows/rust.yml)
[![Codecov](https://codecov.io/gh/l5yth/psn/graph/badge.svg)](https://codecov.io/gh/l5yth/psn)
[![GitHub Release](https://img.shields.io/github/v/release/l5yth/psn)](https://github.com/l5yth/psn/releases)
[![Crates.io](https://img.shields.io/crates/v/psn.svg)](https://crates.io/crates/psn)
[![Top Language](https://img.shields.io/github/languages/top/l5yth/psn)](https://github.com/l5yth/psn)
[![License: Apache-2.0](https://img.shields.io/github/license/l5yth/psn)](https://github.com/l5yth/psn/blob/main/LICENSE)

`psn` is a Rust terminal UI for viewing `ps` process stati and terminating them.

## Dependencies

- any GNU/Linux system with `ps` obviously
- `ps` available in `$PATH`
- Some current Rust stable toolchain (Rust 2024 edition, Cargo)

Core crates: `ratatui`, `crossterm`, `sysinfo`, `nix`, `anyhow`.

## Installation

Helpers exist for Arch and Gentoo-based systems but you can install also
via crates.io or from source directly.

### Archlinux

See [PKGBUILD](./packaging/archlinux/PKGBUILD)

### Gentoo

See [psn-9999.ebuild](./packaging/gentoo/app-misc/psn/psn-9999.ebuild)

### Cargo Crates

```bash
cargo install psn
```

### From Source

Build from source:

```bash
git clone https://github.com/l5yth/psn.git
cd psn
cargo build --release
```

Run the built binary:

```bash
./target/release/psn
```

Or run directly in development:

```bash
cargo run --release --
```

## Usage

```text
psn v0.1.0
apache v2 (c) 2026 l5yth

Usage: psn [OPTIONS]
```

Examples:

```bash
psn
psn "sshd"
```

In-app keys:

- `q`: quit
- `r`: refresh now
- `↑` / `↓`: move selection in service unit list

## Development

```bash
cargo check
cargo test --all --all-features --verbose
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```
