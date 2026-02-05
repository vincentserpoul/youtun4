//! Task management commands.

use tauri::State;
use tracing::{debug, info};

use crate::runtime::TaskId;

use super::state::AppState;

/// Get the status of a running task.
#[tauri::command]
pub async fn get_task_status(
    state: State<'_, AppState>,
    task_id: TaskId,
) -> std::result::Result<Option<String>, String> {
    let status = state.task_status(task_id).await;
    Ok(status.map(|s| format!("{s:?}")))
}

/// Cancel a running task.
///
/// This checks both the async runtime's cancellable tasks and registered
/// download tasks (which use an AtomicBool cancel flag pattern).
#[tauri::command]
pub async fn cancel_task(
    state: State<'_, AppState>,
    task_id: TaskId,
) -> std::result::Result<bool, String> {
    info!("Cancelling task {}", task_id);

    // First try the async runtime's cancel mechanism
    let cancelled = state.runtime().cancel_task(task_id).await;
    if cancelled {
        info!("Task {} cancelled via runtime", task_id);
        return Ok(true);
    }

    // Then try download tasks (which use AtomicBool cancel flags)
    let cancelled = state.cancel_download_task(task_id).await;
    if cancelled {
        info!("Download task {} cancelled successfully", task_id);
        return Ok(true);
    }

    // Then try sync tasks
    let cancelled = state.cancel_sync_task(task_id).await;
    if cancelled {
        info!("Sync task {} cancelled successfully", task_id);
        return Ok(true);
    }

    debug!(
        "Task {} could not be cancelled (not found or already completed)",
        task_id
    );
    Ok(false)
}

/// Get all running tasks count by category.
#[tauri::command]
pub async fn get_running_tasks(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<(String, usize)>, String> {
    let counts = state.runtime().running_tasks_count().await;
    Ok(counts
        .into_iter()
        .map(|(cat, count)| (cat.to_string(), count))
        .collect())
}
