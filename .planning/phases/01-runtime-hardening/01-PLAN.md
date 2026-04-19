---
plan: 01
title: AsyncRuntime overhaul
wave: 1
depends_on: []
files_modified:
  - src-tauri/src/runtime.rs
  - src-tauri/src/commands/state.rs
  - src-tauri/Cargo.toml
  - crates/youtun4-core/src/youtube.rs
  - crates/youtun4-core/src/transfer.rs
autonomous: true
---

# Plan 01: AsyncRuntime overhaul

<objective>
Remove the three concurrency defects in `AsyncRuntime`: (a) `block_on` inside a Tokio context, (b) dropped `JoinHandle` making `cancel_task` unreliable, (c) a parallel `AtomicBool`/`CancelFlag` cancellation path in `commands/state.rs`. Replace with a single `CancellationToken`-based primitive. No user-visible behavior change.
</objective>

<must_haves>
- Single cancellation primitive across the codebase (`tokio_util::sync::CancellationToken`).
- `cancel_task` both signals cancellation AND aborts the underlying task.
- Registering a task never blocks on a runtime.
- All existing Tauri commands compile and pass their tests.
</must_haves>

## Tasks

### Task 1.1 — Write reproducers before touching code

<read_first>
- src-tauri/src/runtime.rs
- src-tauri/Cargo.toml (dev-dependencies)
</read_first>

<action>
Add `src-tauri/tests/runtime_integration.rs`. Add three `#[tokio::test]`s:

1. `spawn_from_async_context` — calls `AsyncRuntime::spawn` from within an async test; currently panics with "Cannot start a runtime from within a runtime". Mark `#[should_panic]` for now; will flip after 1.2.
2. `cancel_aborts_stuck_task` — spawns a task that loops `tokio::time::sleep(Duration::from_millis(50)).await` forever and never checks a cancellation signal. Calls `cancel_task`. Assert the task's `JoinHandle::is_finished()` returns true within 500 ms. Today it would hang — mark `#[ignore = "fails until plan 01 lands"]`.
3. `single_cancellation_primitive` — greps the repo via `std::process::Command` running `rg`, asserting zero hits for `AtomicBool` and `CancelFlag` under `src-tauri/src/`. Mark `#[ignore]` until 1.4.

Add `tokio-util = { version = "0.7", features = ["rt"] }` under `[dev-dependencies]` in `src-tauri/Cargo.toml` if not already present.
</action>

<acceptance_criteria>
- File `src-tauri/tests/runtime_integration.rs` exists.
- `cargo test -p youtun4 --test runtime_integration -- --ignored` lists 3 tests (two ignored, one should_panic).
- `cargo test -p youtun4 --test runtime_integration` passes (the should_panic one).
</acceptance_criteria>

### Task 1.2 — Replace async locks with `parking_lot::Mutex` in `AsyncRuntime`

<read_first>
- src-tauri/src/runtime.rs (entire file, ~560 lines)
- src-tauri/Cargo.toml
</read_first>

<action>
In `src-tauri/src/runtime.rs`:

1. Add `use parking_lot::Mutex;` at the top.
2. Change `tasks: Arc<RwLock<HashMap<TaskId, TaskInfo>>>` to `tasks: Arc<Mutex<HashMap<TaskId, TaskInfo>>>`. Same for `cancel_senders`.
3. Delete the `self.runtime.block_on(async { ... })` blocks at lines 206 and 269. Replace with direct synchronous locking: `self.tasks.lock().insert(task_id, TaskInfo { ... });`.
4. In `cancel_task` and `get_task_info`, replace `.read().await` / `.write().await` with `.lock()`.
5. Make `cancel_task` and `get_task_info` non-async (`pub fn`, not `pub async fn`). Update all callers in `src-tauri/src/commands/` to drop `.await`.

Add `parking_lot = "0.12"` to `src-tauri/Cargo.toml` `[dependencies]` (already in workspace deps? check `Cargo.toml` at repo root first; if yes, use `parking_lot.workspace = true`).
</action>

<acceptance_criteria>
- `rg 'block_on' src-tauri/src/runtime.rs` returns nothing.
- `rg 'RwLock' src-tauri/src/runtime.rs` returns nothing.
- `cargo check -p youtun4` passes.
- Flip `#[should_panic]` on `spawn_from_async_context` to a positive assertion; test passes.
</acceptance_criteria>

### Task 1.3 — Switch cancellation to `CancellationToken` + keep `JoinHandle`

<read_first>
- src-tauri/src/runtime.rs (after 1.2 lands)
- crates/youtun4-core/src/youtube.rs (search for cancellation signal wiring)
- crates/youtun4-core/src/transfer.rs
</read_first>

<action>
In `src-tauri/src/runtime.rs`:

1. Add `tokio-util = { version = "0.7", features = ["rt"] }` to `[dependencies]`.
2. Replace `cancel_senders: Arc<Mutex<HashMap<TaskId, oneshot::Sender<()>>>>` with `cancel_tokens: Arc<Mutex<HashMap<TaskId, CancellationToken>>>`.
3. Extend `TaskInfo` with `handle: Option<tokio::task::JoinHandle<()>>`.
4. In `spawn_cancellable`: create `let token = CancellationToken::new();`, clone for the task body, store both token and `JoinHandle` from `self.runtime.spawn(...)` in the maps.
5. Change the `future_factory` signature from `FnOnce(oneshot::Receiver<()>) -> F` to `FnOnce(CancellationToken) -> F`.
6. In `cancel_task`: look up the token, call `token.cancel()`, look up the handle, call `handle.abort()`. Both are idempotent.
7. Update all call sites in `crates/youtun4-core/src/youtube.rs` and `crates/youtun4-core/src/transfer.rs` that accepted `oneshot::Receiver<()>` to take `CancellationToken` and use `token.is_cancelled()` / `token.cancelled().await` instead of polling the receiver.
</action>

<acceptance_criteria>
- `rg 'oneshot::Receiver|oneshot::Sender' src-tauri/src/runtime.rs crates/youtun4-core/src/` returns nothing.
- `rg 'CancellationToken' src-tauri/src/runtime.rs` shows ≥ 3 hits.
- `cancel_aborts_stuck_task` test passes (un-ignore it).
- `cargo test --workspace` passes.
</acceptance_criteria>

### Task 1.4 — Delete `CancelFlag` / `AtomicBool` cancellation path

<read_first>
- src-tauri/src/commands/state.rs
- src-tauri/src/commands/*.rs (all — grep for `cancel_token`, `cancel_flag`, `CancelFlag`, `AtomicBool`)
</read_first>

<action>
1. Remove the `CancelFlag` type (and any `cancel_token: CancelFlag` field) from `AppState` in `src-tauri/src/commands/state.rs`.
2. Remove methods `cancel_sync_task`, `is_cancelled` (or whatever names are in use).
3. Update every call site to use `AsyncRuntime::cancel_task(task_id)` via the runtime held in `AppState`.
4. If any sync code path was using the `AtomicBool` (non-Tokio context), migrate it to `CancellationToken` — the token is usable from sync code via `is_cancelled()`.
</action>

<acceptance_criteria>
- `rg 'AtomicBool|CancelFlag' src-tauri/src/` returns nothing.
- `cargo test --workspace` passes.
- `single_cancellation_primitive` test passes (un-ignore).
</acceptance_criteria>

### Task 1.5 — Verify & commit

<read_first>
- All files modified above
</read_first>

<action>
Run `just ci`. Fix any fmt/clippy/deny violations. Commit with message:
`refactor(runtime): consolidate cancellation, remove block_on from spawn`
</action>

<acceptance_criteria>
- `just ci` exits 0.
- `git log -1 --name-only` lists the expected files from `files_modified`.
</acceptance_criteria>

## Verification

Beyond acceptance criteria above:
- Manual smoke test: `cargo tauri dev`, start a download, cancel it. Download stops within ~1 second. No tokio panic in logs.
- `cargo nextest run --workspace` green.

## Risks

- `parking_lot::Mutex` is not poison-safe. If a task panics while holding the lock, future `.lock()` calls succeed but read an inconsistent map. **Mitigation:** task registration is 2–3 lines of infallible code; the panic surface is effectively zero.
- Downstream code in `youtube.rs`/`transfer.rs` may check cancellation only at I/O boundaries. Aborting the `JoinHandle` covers the rest. Document this in the `cancel_task` doc comment.
