---
plan: 07
title: Test quality pass — integration tests, fewer mocks
wave: 4
depends_on: [01, 02, 04, 06]
files_modified:
  - crates/youtun4-core/src/**/*.rs (trait definitions)
  - crates/youtun4-core/tests/*.rs (new integration tests)
autonomous: false  # requires judgment — mock-vs-integration calls
---

# Plan 07: Test quality pass

<objective>
For each module with a `mockall::automock`'d trait that has only one production implementor, evaluate whether the trait is earning its keep. Add one `tempfile`-backed integration test per core module to cover real-filesystem paths.
</objective>

<must_haves>
- At least one integration test per core module: `cache`, `cleanup`, `transfer`, `playlist`.
- Traits that exist solely for mocking are deleted and replaced with `#[cfg(test)]` seams.
- `cargo test --workspace` green.
- Coverage (via `just coverage`) for touched modules does not regress.
</must_haves>

## Tasks

### Task 7.1 — Audit `mockall::automock` usage

<read_first>
- `rg '#\[cfg_attr\(test, mockall::automock\)\]|#\[mockall::automock\]' crates/youtun4-core/src/ -l`
- Each file surfaced
</read_first>

<action>
Build a table (commit message or scratch file) of every trait with `automock`:
- Trait name, file:line
- Number of production implementors (count of `impl TraitName for` outside `#[cfg(test)]`)
- Whether tests actually substitute a mock (search for `Mock{TraitName}` in tests)

Decisions per row:
- **Keep** — trait has ≥ 2 prod impls, OR tests meaningfully inject a non-default behavior.
- **Delete** — trait has 1 prod impl and tests only assert "mock was called" tautologies.

No code changes.
</action>

<acceptance_criteria>
- Audit table captured in commit message.
- Every row has a disposition.
</acceptance_criteria>

### Task 7.2 — Delete pointless traits

<read_first>
- Each "Delete"-disposition trait's file
- Its consumers
</read_first>

<action>
For each delete-disposition trait:
1. Inline the sole implementor's methods into the struct that held the trait object (replace `Arc<dyn TraitName>` with the concrete type).
2. Delete the trait definition and the `mockall` attribute.
3. If tests need a seam, introduce it via:
   - Constructor injection of a closure (`type ClockFn = Box<dyn Fn() -> Instant + Send + Sync>;`), or
   - A `#[cfg(test)]`-gated alternative constructor that accepts a test-only collaborator.
</action>

<acceptance_criteria>
- `rg 'mockall::automock' crates/youtun4-core/src/` hit count drops by the number of deleted traits.
- `cargo test --workspace` passes.
- No test imports `Mock{DeletedTrait}` (verified with `rg`).
</acceptance_criteria>

### Task 7.3 — Add integration tests per core module

<read_first>
- `crates/youtun4-core/src/{cache,cleanup,transfer,playlist}.rs` (or module directories after plan 06)
- `crates/youtun4-core/Cargo.toml` (confirm `tempfile` is a dev-dep)
</read_first>

<action>
Add one test file per module, each hitting the real filesystem via `tempfile::tempdir()`:

- `crates/youtun4-core/tests/cache_integration.rs`:
  - Create a cache, add an entry, reopen from disk, assert entry is present.
  - Add 3 entries, exceed a size limit, assert eviction policy picks the right victim.

- `crates/youtun4-core/tests/cleanup_integration.rs`:
  - Create a mock "mounted device" directory, populate with orphan files, run cleanup, assert only orphans are deleted.
  - Run cleanup with `dry_run` flag, assert no files are removed.

- `crates/youtun4-core/tests/transfer_integration.rs`:
  - Copy a 1MB temp file to a temp destination, assert byte-for-byte identical.
  - Copy with a `CancellationToken` that fires after 100ms, assert partial file is cleaned up.

- `crates/youtun4-core/tests/playlist_integration.rs`:
  - Serialize a Playlist to JSON, deserialize, assert equality.
  - Load a fixture playlist file (add a small `tests/fixtures/playlist.json`), assert parsing succeeds with expected entries.
</action>

<acceptance_criteria>
- `ls crates/youtun4-core/tests/` shows ≥ 4 new `*_integration.rs` files.
- `cargo test -p youtun4-core --tests` passes (all unit + integration).
- `cargo nextest run -p youtun4-core` passes.
</acceptance_criteria>

### Task 7.4 — Coverage check (non-blocking)

<read_first>
- Justfile (the `coverage` recipe)
</read_first>

<action>
Run `just coverage` before and after. If any touched module's line coverage dropped by more than 5 points, investigate: likely a code path lost a mock-based test without gaining an integration-test equivalent.

This is informational — don't block on it. Note the result in the commit message.
</action>

<acceptance_criteria>
- Coverage numbers captured in the commit message (before/after, per touched module).
</acceptance_criteria>

### Task 7.5 — Verify & commit

<action>
`just ci`. Commit series — one per module is fine. Final message: `test: add filesystem integration tests; remove unused mock traits`
</action>

<acceptance_criteria>
- `just ci` exits 0.
- Phase-wide `must_haves` from PHASE.md all check out.
</acceptance_criteria>

## Risks

- Filesystem integration tests can be flaky on CI (permission issues, disk pressure). Use `tempfile::tempdir()` (auto-cleanup) rather than hardcoded paths, and keep test sizes small (≤ 1MB).
- Deleting a trait that another consumer wanted for DI is a breaking change *within* the workspace. Only external consumer is `src-tauri`, which we control. Safe.
