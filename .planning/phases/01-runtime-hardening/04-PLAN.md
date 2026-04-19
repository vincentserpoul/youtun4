---
plan: 04
title: Error modeling — move retryability into the type system
wave: 2
depends_on: [01]
files_modified:
  - crates/youtun4-core/src/error.rs
  - crates/youtun4-core/src/youtube.rs
  - crates/youtun4-core/src/transfer.rs
  - crates/youtun4-core/src/queue.rs
  - src-tauri/src/commands/error.rs
autonomous: true
---

# Plan 04: Error modeling — retryability as types, not methods

<objective>
Replace the current `DownloadError::Network { ... }` + `Error::is_retryable()` / `retry_delay_secs()` pattern with variants that encode retryability in the type. Callers pattern-match; impossible to forget a case.
</objective>

<must_haves>
- `Error::is_retryable()` and `Error::retry_delay_secs()` methods are deleted.
- Every call site that previously branched on retryability now pattern-matches variants.
- Downstream Tauri serialization carries structured retry info instead of free-form strings.
</must_haves>

## Tasks

### Task 4.1 — Inventory current retry branches

<read_first>
- crates/youtun4-core/src/error.rs
- crates/youtun4-core/src/queue.rs (queue owns retry logic)
- crates/youtun4-core/src/youtube.rs (emits errors)
</read_first>

<action>
Grep for all callers of `.is_retryable()` and `.retry_delay_secs()`. For each, record: file, line, what the caller does with the result.

No code changes in this task.
</action>

<acceptance_criteria>
- `rg '\.is_retryable\(\)|\.retry_delay_secs\(\)' crates/ src-tauri/` output is captured in the commit message or a scratch file.
</acceptance_criteria>

### Task 4.2 — Split `DownloadError::Network`

<read_first>
- crates/youtun4-core/src/error.rs (the `DownloadError` enum, lines ~150–240)
</read_first>

<action>
In `crates/youtun4-core/src/error.rs`:

1. Replace the `Network { message, source }` variant with two:

```rust
/// Transient network error — safe to retry.
#[error("transient network error: {message} (retry after {retry_after:?})")]
TransientNetwork {
    message: String,
    retry_after: std::time::Duration,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
},

/// Permanent network/HTTP error — do not retry.
#[error("permanent network error: {message}")]
PermanentNetwork {
    message: String,
    reason: PermanentNetworkReason,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
},
```

2. Add `PermanentNetworkReason` enum: `GeoBlocked`, `Forbidden403`, `NotFound404`, `TlsHandshakeFailure`, `Other`.

3. `Timeout` and `RateLimited` already encode retry info — leave them as transient-by-construction. Add a doc comment making it explicit.

4. Audit the mapping from `rusty_ytdl` / `reqwest` errors into `DownloadError` (probably in `youtube.rs`). Route 5xx / connection-refused / DNS → `TransientNetwork`. Route 403 / 404 / geo-blocked → `PermanentNetwork`.
</action>

<acceptance_criteria>
- `rg 'DownloadError::Network\b' crates/` returns nothing (the single variant is gone).
- `rg 'TransientNetwork|PermanentNetwork' crates/youtun4-core/src/error.rs` returns ≥ 2 hits.
- `cargo check -p youtun4-core` passes.
</acceptance_criteria>

### Task 4.3 — Delete `is_retryable` / `retry_delay_secs` methods

<read_first>
- crates/youtun4-core/src/error.rs (the `Error` impl block)
- Files from 4.1 inventory
</read_first>

<action>
1. Delete the `is_retryable()` and `retry_delay_secs()` methods on `Error` in `error.rs`.
2. At every call site from 4.1, replace the method call with a `match` on the specific error variant:

```rust
match err {
    Error::Download(DownloadError::TransientNetwork { retry_after, .. })
    | Error::Download(DownloadError::Timeout { .. }) => {
        tokio::time::sleep(*retry_after).await;
        // retry
    }
    Error::Download(DownloadError::RateLimited { retry_after_secs }) => {
        tokio::time::sleep(Duration::from_secs(*retry_after_secs)).await;
        // retry
    }
    Error::Download(DownloadError::PermanentNetwork { .. })
    | Error::Download(DownloadError::VideoUnavailable { .. }) => {
        // do not retry — surface to user
    }
    _ => { /* ... */ }
}
```

3. Add `#[deny(non_exhaustive_patterns)]` or ensure the match is exhaustive so adding future variants is compile-checked.
</action>

<acceptance_criteria>
- `rg 'is_retryable|retry_delay_secs' crates/ src-tauri/` returns nothing.
- `cargo test --workspace` passes.
- `cargo clippy --workspace -- -D warnings` passes.
</acceptance_criteria>

### Task 4.4 — Update Tauri error serialization

<read_first>
- src-tauri/src/commands/error.rs
</read_first>

<action>
In `src-tauri/src/commands/error.rs`, the current code likely stringifies `e.kind()` via `format!("{:?}", ...)`. Replace with an explicit serializable shape:

```rust
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ErrorPayload {
    TransientNetwork { message: String, retry_after_ms: u64 },
    PermanentNetwork { message: String, reason: String },
    Timeout { title: String, timeout_secs: u64 },
    RateLimited { retry_after_secs: u64 },
    VideoUnavailable { video_id: String, reason: String },
    // ... one variant per DownloadError
    Other { message: String },
}
```

The frontend can now pattern-match on `kind` instead of regexing a message string.
</action>

<acceptance_criteria>
- `rg 'format!\("{:\?}"' src-tauri/src/commands/error.rs` returns nothing (no debug-formatted errors sent to frontend).
- `cargo test -p youtun4` passes.
</acceptance_criteria>

### Task 4.5 — Verify & commit

<action>
`just ci`. Commit: `refactor(error): encode retryability in the type system`
</action>

<acceptance_criteria>
- `just ci` exits 0.
- Test suite green.
</acceptance_criteria>

## Risks

- Frontend code may be regex-parsing error messages. If it breaks, that's a bug we're surfacing, not introducing — fix forward by switching the frontend to the structured `kind` field.
- `#[non_exhaustive]` on public error enums is a semver concern for this workspace (internal only — no external consumers). If external crates were to depend on `youtun4-core`, revisit.
