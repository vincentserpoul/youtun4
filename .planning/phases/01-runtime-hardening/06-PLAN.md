---
plan: 06
title: Split youtube.rs and oversized core modules
wave: 3
depends_on: [01, 04]
files_modified:
  - crates/youtun4-core/src/youtube.rs (deleted)
  - crates/youtun4-core/src/youtube/mod.rs (new)
  - crates/youtun4-core/src/youtube/fetch.rs (new)
  - crates/youtun4-core/src/youtube/download.rs (new)
  - crates/youtun4-core/src/youtube/transcode.rs (new)
  - crates/youtun4-core/src/youtube/progress.rs (new)
  - crates/youtun4-core/src/lib.rs
autonomous: true
---

# Plan 06: Split `youtube.rs` and oversized core modules

<objective>
Pure refactor: move code from the 3,335-line `youtube.rs` into focused submodules. No logic change. One commit per file moved for easy review/revert.
</objective>

<must_haves>
- `youtube.rs` (single file) is gone, replaced by a `youtube/` directory.
- No file in `crates/youtun4-core/src/` exceeds 1000 LOC after the refactor.
- Public API of `youtun4-core` unchanged (same `pub use` surface).
- `cargo test --workspace` green at every intermediate commit.
</must_haves>

## Tasks

### Task 6.1 — Establish submodule skeleton

<read_first>
- crates/youtun4-core/src/youtube.rs (first 100 lines for imports/structure)
- crates/youtun4-core/src/lib.rs (how youtube is re-exported)
</read_first>

<action>
1. Create directory `crates/youtun4-core/src/youtube/`.
2. Create `crates/youtun4-core/src/youtube/mod.rs` with:
   ```rust
   mod fetch;
   mod download;
   mod transcode;
   mod progress;

   pub use fetch::*;
   pub use download::*;
   pub use progress::*;
   // transcode is internal — not re-exported
   ```
3. Leave `youtube.rs` in place for now. This task only creates the scaffolding.
4. Do NOT delete `youtube.rs` yet — Rust will refuse to compile with both `youtube.rs` and `youtube/mod.rs`. So: rename `youtube.rs` → `youtube/_legacy.rs` and have `mod.rs` re-export from `_legacy` temporarily. Gradual migration.

Actually simpler: make 6.1 a single commit that renames `youtube.rs` to `youtube/mod.rs` with no other changes. Then subsequent tasks extract sections.
</action>

<acceptance_criteria>
- `ls crates/youtun4-core/src/youtube/` shows `mod.rs` (equal in line count to the old `youtube.rs`).
- `crates/youtun4-core/src/youtube.rs` does not exist.
- `cargo build --workspace` passes.
- `cargo test --workspace` passes.
- Commit: `refactor(youtube): convert to submodule (no logic change)`
</acceptance_criteria>

### Task 6.2 — Extract `progress.rs`

<read_first>
- crates/youtun4-core/src/youtube/mod.rs (the old content)
</read_first>

<action>
Move the `DownloadProgressTracker` struct (and its `impl` block, plus the helper types it owns — `SpeedSample`, etc., roughly lines 580–760 of the old file) into `crates/youtun4-core/src/youtube/progress.rs`. Add `mod progress; pub use progress::*;` to `mod.rs` (already done in 6.1).

No logic changes. Just cut-and-paste with the minimum imports needed to compile.
</action>

<acceptance_criteria>
- `wc -l crates/youtun4-core/src/youtube/progress.rs` shows a non-trivial file (≥ 100 lines).
- The moved items are no longer in `mod.rs`.
- `cargo test --workspace` passes.
- Commit: `refactor(youtube): extract progress tracker into its own module`
</acceptance_criteria>

### Task 6.3 — Extract `transcode.rs` (AAC→MP3)

<read_first>
- crates/youtun4-core/src/youtube/mod.rs
</read_first>

<action>
Move the audio transcoding logic (AAC→MP3 conversion, roughly lines 900–1200 of the original file) into `crates/youtun4-core/src/youtube/transcode.rs`. Keep it private to the `youtube` module — no `pub use`.
</action>

<acceptance_criteria>
- `wc -l crates/youtun4-core/src/youtube/transcode.rs` ≥ 100 lines.
- `rg 'fn.*aac|fn.*mp3|fn.*transcode' crates/youtun4-core/src/youtube/mod.rs` returns nothing.
- `cargo test --workspace` passes.
- Commit: `refactor(youtube): extract transcode helpers`
</acceptance_criteria>

### Task 6.4 — Extract `fetch.rs` (URL validation, playlist parsing)

<read_first>
- crates/youtun4-core/src/youtube/mod.rs
</read_first>

<action>
Move URL validation, playlist-ID parsing, and playlist metadata fetching (the pre-download portion) into `crates/youtun4-core/src/youtube/fetch.rs`.
</action>

<acceptance_criteria>
- `cargo test --workspace` passes.
- Commit: `refactor(youtube): extract fetch/validation helpers`
</acceptance_criteria>

### Task 6.5 — Extract `download.rs` (orchestration)

<read_first>
- crates/youtun4-core/src/youtube/mod.rs
</read_first>

<action>
Move the `RustyYtdlDownloader` struct and its download-orchestration `impl` into `crates/youtun4-core/src/youtube/download.rs`. `mod.rs` is now mostly re-exports and shared types.
</action>

<acceptance_criteria>
- `wc -l crates/youtun4-core/src/youtube/mod.rs` < 400 lines.
- `wc -l crates/youtun4-core/src/youtube/download.rs` < 1000 lines.
- `cargo test --workspace` passes.
- Commit: `refactor(youtube): extract download orchestration`
</acceptance_criteria>

### Task 6.6 — Check remaining oversized modules

<read_first>
- `wc -l crates/youtun4-core/src/*.rs`
</read_first>

<action>
If `cache.rs` or `cleanup.rs` still exceeds 1000 LOC, apply the same pattern:
- `cache.rs` → `cache/{mod,manifest,cleanup,storage}.rs` (split by concern, not by size).
- `cleanup.rs` → `cleanup/{mod,device,orphan}.rs`.

Only do this if over 1000 LOC after plans 01–05 land. If under, skip.
</action>

<acceptance_criteria>
- `find crates/youtun4-core/src -name '*.rs' -exec wc -l {} + | awk '$1 > 1000'` returns nothing.
- `just ci` passes.
</acceptance_criteria>

### Task 6.7 — Verify & final commit

<action>
Final `just ci`. Squash-merge or preserve per-task commits as you prefer. Repo-wide grep: `rg 'pub use .* youtube' crates/youtun4-core/src/lib.rs` should show the re-export line(s).
</action>

<acceptance_criteria>
- `just ci` exits 0.
- `cargo doc --workspace --no-deps` builds without warnings about missing docs on public items.
</acceptance_criteria>

## Risks

- Rust's `mod.rs` discovery: moving a file changes the module path. Any `#[cfg(test)]` blocks with `use super::*;` keep working; external consumers might have `use youtun4_core::youtube::SomeType;` that breaks if we forget a `pub use`. The `pub use *::*;` in `mod.rs` mitigates this for anything already `pub`.
- Intermediate commits must each compile. If an extraction leaves behind dangling references in `mod.rs`, fix in the same commit — don't push broken states.
