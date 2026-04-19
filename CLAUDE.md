# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Youtun4 is a Tauri 2.x desktop app for managing MP3 playlists from YouTube. It detects USB MP3 players, downloads YouTube playlists as MP3, and syncs them to devices. The frontend is Leptos (Rust compiled to WASM) and the backend is pure Rust.

## Build & Development Commands

Uses `just` as task runner (see `Justfile` for all recipes):

```bash
just check          # Quick compile check
just build          # Debug build
just build-release  # Release build
just test           # Run tests with nextest
just test-one NAME  # Run a single test by name
just test-doc       # Run doctests
just clippy         # Run clippy lints
just clippy-fix     # Auto-fix clippy issues
just fmt            # Format with nightly rustfmt
just fmt-check      # Check formatting
just coverage       # Code coverage with llvm-cov
just dev            # Quick local iteration: check + clippy + test
just ci             # Full CI suite: fmt-check + clippy + test + doc-check + deny + audit + machete
cargo tauri dev     # Run the full app in development mode
cargo tauri build   # Build the distributable app
trunk build         # Build the WASM frontend (output to dist/)
```

## Workspace Architecture

Three workspace members with a clear dependency hierarchy:

- **`crates/youtun4-core`** — Pure Rust core library. All business logic: device detection, playlist management, YouTube downloading (via `rusty_ytdl`), file transfer, cache, integrity verification, sync orchestration, download queue. No UI or framework dependencies. Uses `mockall` for trait mocking in tests.

- **`crates/youtun4-ui`** — Leptos components compiled to WASM. Client-side rendered (`csr` feature). Communicates with the Tauri backend via IPC (`invoke`). Built by Trunk (config in `Trunk.toml`, entry at `crates/youtun4-ui/index.html`, output to `dist/`).

- **`src-tauri`** — Tauri application shell. Contains IPC command handlers in `src/commands/` (one module per feature area), app state management, logging setup, and an `AsyncRuntime` wrapper (`src/runtime.rs`) for task spawning/cancellation/progress tracking. Commands re-export from `src/commands/mod.rs`.

## Lint & Code Quality Rules

Strict lint configuration in workspace `Cargo.toml`:

- **`unsafe_code = "deny"`** — no unsafe code
- **`unwrap_used = "deny"`, `expect_used = "deny"`** — use proper error handling (`.ok()`, `?`, `map_err`)
- **`panic = "deny"`, `unreachable = "deny"`, `unimplemented = "deny"`**
- **`print_stdout = "deny"`, `print_stderr = "deny"`, `dbg_macro = "deny"`** — use `tracing` instead
- **`indexing_slicing = "deny"`** — use `.get()` with bounds checking
- **`float_arithmetic = "deny"`, `cast_possible_truncation = "deny"`, `cast_sign_loss = "deny"`**
- Pedantic clippy enabled across the board
- Tests are relaxed: `clippy.toml` allows `unwrap`, `expect`, `panic`, `dbg`, `print`, and indexing in tests

## Toolchain

- Rust 1.95.0 (pinned in `rust-toolchain.toml`)
- Edition 2024
- Nightly required for `rustfmt` only (`cargo +nightly fmt`)
- WASM target: `wasm32-unknown-unknown`

## Pre-commit Hooks

Configured in `.pre-commit-config.yaml` (uses `prek`): cargo fmt, cargo clippy, cargo deny, prettier, markdownlint, gitleaks secret detection.
