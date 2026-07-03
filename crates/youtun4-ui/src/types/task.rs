use serde::{Deserialize, Serialize};

/// Unique identifier for a spawned task.
pub type TaskId = u64;

/// Running task count by category.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskCount {
    /// Task category name.
    pub category: String,
    /// Number of running tasks in this category.
    pub count: usize,
}
