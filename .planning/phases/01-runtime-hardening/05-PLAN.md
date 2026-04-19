---
plan: 05
title: IPC boundary contract — catch DTO drift at compile time
wave: 2
depends_on: [01]
files_modified:
  - src-tauri/src/commands/youtube.rs
  - src-tauri/src/commands/device.rs
  - src-tauri/src/commands/playlist.rs
  - src-tauri/src/commands/transfer.rs
  - src-tauri/tests/ipc_contract.rs
autonomous: true
---

# Plan 05: IPC boundary contract

<objective>
Replace hand-written `impl From<core::T> for DtoT` conversions with a round-trip property test per command, so adding a field in `youtun4-core` fails CI when the DTO is missed.
</objective>

<must_haves>
- Every command that returns or accepts a core type has a round-trip test.
- Adding a field to the core type without updating the DTO causes the test to fail (verified manually once).
- No production code change required beyond removing duplicated conversion logic where the types are already isomorphic.
</must_haves>

## Tasks

### Task 5.1 — Inventory DTOs

<read_first>
- src-tauri/src/commands/*.rs (every command module)
- crates/youtun4-core/src/*.rs (the core types being mirrored)
</read_first>

<action>
Build a table (in the commit message) of `(command_fn, core_type, dto_type, fields_count_match)`. For each, decide:
- **A:** Core type is already `Serialize` and DTO is a pointless copy — delete the DTO, return the core type directly.
- **B:** DTO exists because field names or shapes legitimately differ for the frontend — keep DTO, add round-trip test.
- **C:** DTO adds fields the core type doesn't have (computed/derived) — keep DTO, add test for the shared fields only.

No code changes yet.
</action>

<acceptance_criteria>
- Commit message or scratch note contains the table with one row per `(core_type, dto_type)` pair.
- Every pair has a disposition (A/B/C).
</acceptance_criteria>

### Task 5.2 — Delete redundant DTOs (disposition A)

<read_first>
- Each file with a disposition-A pair
</read_first>

<action>
For each disposition-A pair:
1. Remove the `#[derive(Serialize)]` DTO struct.
2. Change the command signature to return the core type directly: `#[tauri::command] fn foo() -> Result<CorePayload, ErrorPayload>`.
3. Confirm `youtun4-core::CorePayload` derives `Serialize` (add if missing).
</action>

<acceptance_criteria>
- `cargo build --workspace` passes.
- `cargo test --workspace` passes.
- The removed DTO types are not referenced anywhere (check with `rg`).
</acceptance_criteria>

### Task 5.3 — Round-trip test harness

<read_first>
- src-tauri/src/commands/*.rs
- src-tauri/Cargo.toml (dev-dependencies)
</read_first>

<action>
Create `src-tauri/tests/ipc_contract.rs`. For each remaining disposition-B or -C pair, add a test:

```rust
#[test]
fn roundtrip_download_progress() {
    let core = youtun4_core::DownloadProgress {
        // populate every field with a non-default value
        url: "https://youtube.com/watch?v=abc".into(),
        status: youtun4_core::DownloadStatus::InProgress,
        bytes_downloaded: 12345,
        total_bytes: Some(67890),
        // ... etc — use all-non-default values so a missed field fails below
    };
    let dto: DownloadProgressPayload = (&core).into();
    // Shared fields must match:
    assert_eq!(dto.url, core.url);
    assert_eq!(dto.bytes_downloaded, core.bytes_downloaded);
    // ... one assert per shared field
    // Round-trip through JSON to catch serde shape drift:
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("\"url\":\"https://youtube.com/watch?v=abc\""));
    assert!(json.contains("\"bytes_downloaded\":12345"));
}
```

Add `serde_json = "1"` to `[dev-dependencies]` if not already present.
</action>

<acceptance_criteria>
- `cargo test -p youtun4 --test ipc_contract` passes with ≥ 1 test per disposition-B/C pair.
- Temporarily adding a field to one core type (without touching DTO) and re-running the test suite fails with a clear error (verified manually, reverted).
</acceptance_criteria>

### Task 5.4 — Verify & commit

<action>
`just ci`. Commit: `test(ipc): add round-trip contracts between core and tauri DTOs`
</action>

<acceptance_criteria>
- `just ci` exits 0.
</acceptance_criteria>

## Risks

- Manual field enumeration in tests gets stale. An automated alternative (proc macro, or using `serde`'s introspection via `schemars`) is tempting but adds dep weight. Keep manual tests unless drift causes ≥ 2 production bugs — then revisit.
