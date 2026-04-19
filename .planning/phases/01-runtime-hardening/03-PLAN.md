---
plan: 03
title: Framework-free dependency enforcement
wave: 1
depends_on: []
files_modified:
  - crates/youtun4-core/Cargo.toml
  - crates/youtun4-core/src/device.rs
  - src-tauri/Cargo.toml
  - src-tauri/src/commands/device.rs
  - deny.toml
autonomous: true
---

# Plan 03: Framework-free dependency enforcement

<objective>
Remove `reqwest` from `youtun4-core` (per the standing TODO at `crates/youtun4-core/Cargo.toml:23`), move platform mount/unmount logic out of core into `src-tauri`, and add `cargo-deny` bans preventing re-introduction.
</objective>

<must_haves>
- `reqwest` is no longer a direct dep of `youtun4-core`.
- `cargo deny check bans` passes, and re-adding `reqwest` to `youtun4-core/Cargo.toml` causes it to fail.
- Platform-specific mount code (`#[cfg(target_os = ...)]`) lives in `src-tauri`, not `youtun4-core`.
- Device *detection* trait remains in core; only the mount *implementation* moves.
</must_haves>

## Tasks

### Task 3.1 — Prove `reqwest` is removable

<read_first>
- crates/youtun4-core/Cargo.toml
- crates/youtun4-core/src/*.rs (grep for `reqwest::`)
</read_first>

<action>
Grep all `reqwest::` uses in `youtun4-core/src/`. For each, determine: (a) can this use rusty_ytdl's embedded HTTP client, (b) is it a thumbnail fetch that can move to src-tauri, or (c) is it unused/dead code.

Write findings as a comment block in the commit message. No code changes in this task.
</action>

<acceptance_criteria>
- `rg 'reqwest::' crates/youtun4-core/src/ -l` output is captured in the audit.
- Each hit has a documented disposition in the commit note.
</acceptance_criteria>

### Task 3.2 — Remove `reqwest` dep and migrate call sites

<read_first>
- crates/youtun4-core/Cargo.toml
- Each file identified in 3.1
</read_first>

<action>
For each call site identified in 3.1: apply the disposition. Most likely paths:
- Thumbnail fetches → move the function body to a new helper in `src-tauri/src/commands/thumbnail.rs`; core exposes only the URL, not the bytes.
- `rusty_ytdl`-adjacent → delete; rusty_ytdl already has its own client.

Delete the `reqwest = ...` line and the TODO comment above it in `crates/youtun4-core/Cargo.toml`.
</action>

<acceptance_criteria>
- `rg 'reqwest' crates/youtun4-core/` returns nothing.
- `cargo check -p youtun4-core` passes.
- `cargo test -p youtun4-core` passes.
- `cargo build --workspace` passes (src-tauri still builds with its own reqwest if it pulled one in).
</acceptance_criteria>

### Task 3.3 — Move platform mount logic to src-tauri

<read_first>
- crates/youtun4-core/src/device.rs (full file)
- src-tauri/src/commands/device.rs (full file)
</read_first>

<action>
1. In `crates/youtun4-core/src/device.rs`, ensure the `DeviceMountHandler` trait remains (trait-only, no impl).
2. Move the concrete `PlatformMountHandler` struct (or whatever the `#[cfg(target_os = ...)]`-gated impl is called) to `src-tauri/src/commands/device.rs` (or a new `src-tauri/src/platform_mount.rs`).
3. Update `src-tauri` code that was constructing `PlatformMountHandler` from core to construct it locally.
4. Confirm `crates/youtun4-core/src/device.rs` has zero `#[cfg(target_os = ...)]` attributes.
</action>

<acceptance_criteria>
- `rg 'cfg\(target_os' crates/youtun4-core/src/device.rs` returns nothing.
- `rg 'PlatformMountHandler|mount_device|unmount_device' crates/youtun4-core/src/` returns nothing (impl-wise — trait method names can remain).
- `cargo build --workspace` passes.
- `cargo test --workspace` passes.
</acceptance_criteria>

### Task 3.4 — Enforce via `cargo-deny`

<read_first>
- deny.toml (repo root — create if missing)
</read_first>

<action>
Add or extend `[bans]` section in `deny.toml`:

```toml
[[bans.deny]]
name = "reqwest"
wrappers = []  # no exceptions

# Restrict to src-tauri / tools only; core must not pull reqwest
[[bans.features]]
name = "reqwest"
# (feature-level bans aren't per-crate; use workspace `deny` with a custom check below)
```

Since `cargo-deny` doesn't natively support "ban crate X only in workspace member Y", enforce the rule with a CI check: add a `justfile` recipe `check-core-purity`:

```
check-core-purity:
    @! cargo tree -p youtun4-core --edges=normal 2>/dev/null | grep -E '^\s*(reqwest|tauri|sysinfo_platform)' || (echo "youtun4-core has banned deps" && exit 1)
```

And wire it into `just ci` so it runs in CI.

(Note: `sysinfo` stays in core for now — it provides portable disk enumeration via a crate, not a platform API. Revisit if it leaks types.)
</action>

<acceptance_criteria>
- `just check-core-purity` exits 0.
- Temporarily re-adding `reqwest = "0.13"` to `crates/youtun4-core/Cargo.toml` causes `just check-core-purity` to exit non-zero (test manually, revert).
- `just ci` runs the new check.
</acceptance_criteria>

### Task 3.5 — Verify & commit

<action>
`just ci`. Commit: `refactor(core): remove reqwest, move platform mounts to src-tauri`
</action>

<acceptance_criteria>
- `just ci` exits 0.
- `git diff --stat main...HEAD` touches only files in `files_modified`.
</acceptance_criteria>

## Risks

- `rusty_ytdl`'s client may not cover all use cases `reqwest` was covering (e.g., follow-redirect semantics, custom user-agent). If 3.2 reveals a hard dependency, document in `crates/youtun4-core/src/README.md` (or module doc) why `reqwest` had to stay, and close with `#[allow(dead_code)]` on the banned-deps check — don't fake compliance.
