//! Task Management API.

use crate::types::{TaskCount, TaskId};

use super::invoke;

/// Get the status of a running task.
///
/// Returns the status as a string (e.g., "Running", "Completed", "Failed(error)", "Cancelled").
pub async fn get_task_status(task_id: TaskId) -> Result<Option<String>, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        task_id: TaskId,
    }

    invoke("get_task_status", Args { task_id }).await
}

/// Get all running tasks count by category.
pub async fn get_running_tasks() -> Result<Vec<TaskCount>, String> {
    #[derive(serde::Serialize)]
    #[allow(
        clippy::empty_structs_with_brackets,
        reason = "Serde requires braces for empty struct to serialize as {}"
    )]
    struct Args {}

    let result: Vec<(String, usize)> = invoke("get_running_tasks", Args {}).await?;
    Ok(result
        .into_iter()
        .map(|(category, count)| TaskCount { category, count })
        .collect())
}

/// Cancel a running task.
///
/// Returns `true` if the task was successfully cancelled, `false` otherwise.
pub async fn cancel_task(task_id: TaskId) -> Result<bool, String> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        task_id: TaskId,
    }

    invoke("cancel_task", Args { task_id }).await
}
