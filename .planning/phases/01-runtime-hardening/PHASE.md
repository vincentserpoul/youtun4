# Phase 01: Runtime & Core Hardening

**Created:** 2026-04-19
**Source:** Staff-level audit of youtun4 codebase
**Status:** Ready to execute

## Goal

Close the structural defects identified in the audit — concurrency bugs, data-integrity risks, leaky architectural boundaries, and oversized modules — without changing user-visible behavior.

## Non-goals

- No new features.
- No UI changes.
- No rewrites. Each plan is a targeted, reversible edit.

## Must-haves (goal-backward verification)

- [ ] `rg 'block_on' src-tauri/src/runtime.rs` returns nothing outside `#[cfg(test)]`.
- [ ] `rg 'AtomicBool|CancelFlag' src-tauri/` returns nothing (single cancellation primitive).
- [ ] No `.unwrap()` / `.expect()` introduced (workspace lints already enforce).
- [ ] `cargo test --workspace` green.
- [ ] `just ci` green (fmt + clippy + deny + audit + machete).
- [ ] Every file in `crates/youtun4-core/src/*.rs` is ≤ 1000 LOC.
- [ ] `cargo deny check bans` passes with new bans on `reqwest`/`sysinfo` in `youtun4-core`.
- [ ] At least one `tempfile`-backed integration test per core module (cache, cleanup, transfer, playlist).

## Waves

| Wave | Plans | Parallel? | Rationale |
|------|-------|-----------|-----------|
| 1    | 01, 02, 03 | Yes | Touch disjoint files (runtime, cache, Cargo.toml). Ship first — highest risk reduction. |
| 2    | 04, 05 | Yes | Depend on 01 landing (error types + IPC touch runtime). Disjoint within wave. |
| 3    | 06 | Alone | Pure move refactor; want clean types from 04 before splitting. |
| 4    | 07 | Alone | Ongoing; best after structure is stable. |

## Plans

| # | Title | Risk | Scope |
|---|-------|------|-------|
| 01 | AsyncRuntime overhaul | 🔴 Critical | `src-tauri/src/runtime.rs`, `src-tauri/src/commands/state.rs` |
| 02 | Cache manifest atomicity | 🔴 Data integrity | `crates/youtun4-core/src/cache.rs` |
| 03 | Framework-free dep enforcement | 🟠 Arch | `Cargo.toml`, `deny.toml`, `device.rs` |
| 04 | Error modeling refactor | 🟡 Correctness | `crates/youtun4-core/src/error.rs` + all callers |
| 05 | IPC boundary contract | 🟡 Maintainability | `src-tauri/src/commands/*` |
| 06 | Split `youtube.rs` | 🟡 Clarity | `crates/youtun4-core/src/youtube.rs` → `youtube/` |
| 07 | Test quality pass | 🟢 Hygiene | Across `crates/youtun4-core/**` |

## Rollback strategy

Each plan is an atomic commit (or small commit series) on its own branch off `main`. If any plan's test suite regresses on `main`, revert that plan's commits; later plans on the same wave remain valid.

## Out of scope (deferred)

- Replacing `rusty_ytdl` entirely.
- Rewriting the WASM frontend.
- Adding new platform support.
- Performance tuning (measure after hardening lands).
