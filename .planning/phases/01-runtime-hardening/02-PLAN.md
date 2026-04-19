---
plan: 02
title: Cache manifest atomicity
wave: 1
depends_on: []
files_modified:
  - crates/youtun4-core/src/cache.rs
  - crates/youtun4-core/Cargo.toml
autonomous: true
---

# Plan 02: Cache manifest atomicity

<objective>
Ensure `cache_manifest.json` writes are atomic: write to a temp file, fsync, then rename over the target. Eliminates the silent-corruption window on crash or concurrent write.
</objective>

<must_haves>
- Manifest writes use write-temp-then-rename.
- A crash-during-write test demonstrates the old manifest survives.
- No behavior change when writes succeed normally.
</must_haves>

## Tasks

### Task 2.1 — Audit current write path

<read_first>
- crates/youtun4-core/src/cache.rs (full file)
- crates/youtun4-core/Cargo.toml
</read_first>

<action>
Read every call site that writes `cache_manifest.json`. Note whether `tempfile` or `atomicwrites` is already a dep (check `Cargo.toml`). If a helper exists, we'll extend it; otherwise we'll introduce one.

No code changes in this task — just identify the 1–3 write sites and note their function names.
</action>

<acceptance_criteria>
- Comment in the commit message (or a scratch note) lists each write site by `file:line` and function name.
- `grep -n "cache_manifest" crates/youtun4-core/src/cache.rs` result matches the audit.
</acceptance_criteria>

### Task 2.2 — Add atomic-write helper

<read_first>
- crates/youtun4-core/src/cache.rs
- crates/youtun4-core/Cargo.toml (dev-deps show `tempfile` is already present)
</read_first>

<action>
In `crates/youtun4-core/Cargo.toml`, move `tempfile` from `[dev-dependencies]` to `[dependencies]` (keep the dev entry if other test-only usage exists).

In `cache.rs`, add a private helper:

```rust
fn write_json_atomic<P: AsRef<std::path::Path>, T: serde::Serialize>(
    path: P,
    value: &T,
) -> Result<(), CacheError> {
    let path = path.as_ref();
    let parent = path.parent().ok_or_else(|| CacheError::Io("manifest has no parent dir".into()))?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| CacheError::Io(e.to_string()))?;
    serde_json::to_writer(&mut tmp, value).map_err(|e| CacheError::Serialize(e.to_string()))?;
    tmp.as_file().sync_all().map_err(|e| CacheError::Io(e.to_string()))?;
    tmp.persist(path).map_err(|e| CacheError::Io(e.error.to_string()))?;
    Ok(())
}
```

Replace every manifest write site identified in 2.1 with a call to `write_json_atomic(&self.manifest_path, &manifest)`.
</action>

<acceptance_criteria>
- `rg 'fs::write|File::create' crates/youtun4-core/src/cache.rs` shows no writes to `cache_manifest.json` (only reads, or writes to cache payload files).
- `rg 'write_json_atomic' crates/youtun4-core/src/cache.rs` shows ≥ 2 hits (definition + ≥ 1 call).
- `cargo test -p youtun4-core` passes.
</acceptance_criteria>

### Task 2.3 — Crash-during-write integration test

<read_first>
- crates/youtun4-core/src/cache.rs (after 2.2)
- crates/youtun4-core/tests/ (existing test layout)
</read_first>

<action>
Add `crates/youtun4-core/tests/cache_atomic.rs` with a `#[test]`:

1. Create `tempfile::tempdir()`.
2. Seed a valid `cache_manifest.json` with a known entry: `{"version":1,"entries":[{"id":"A"}]}`.
3. Simulate a failed write: create a stray orphan temp file in the cache dir with invalid JSON (`cache.tmp.broken`). This mimics a process killed between `NamedTempFile::new_in` and `.persist`.
4. Load the manifest via the public `CacheManager::load` (or equivalent). Assert the returned manifest still has entry `"A"` — the orphan temp file did not replace it.
5. Also assert the orphan file was either cleaned up or ignored (not loaded as manifest).
</action>

<acceptance_criteria>
- `cargo test -p youtun4-core --test cache_atomic` passes.
- Removing the atomic-write helper and using `fs::write` instead causes the test to pass OR fail (either is fine — it's a correctness regression test, not a crash reproducer). Document which in a comment on the test.
</acceptance_criteria>

### Task 2.4 — Verify & commit

<read_first>
- Files modified above
</read_first>

<action>
`just ci`. Commit: `fix(cache): atomic manifest writes via tempfile + rename`
</action>

<acceptance_criteria>
- `just ci` exits 0.
- `cargo test --workspace` green.
</acceptance_criteria>

## Risks

- macOS APFS and Windows NTFS both support atomic rename within the same volume. Confirm the temp file is created in the same directory as the target (via `NamedTempFile::new_in(parent)`, not `::new()` which uses `/tmp`).
- If the cache dir is on a different volume than the system tempdir, `::new()` would break the atomicity guarantee. `new_in(parent)` avoids this.
