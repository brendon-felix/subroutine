use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Template grouping actions into sequences (ordered or randomizable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_sequential: bool,
    pub allow_randomization: bool,
    pub default_start_time: Option<String>,
    pub default_end_time: Option<String>,
    pub created_at: Option<String>,
}

impl Routine {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: None,
            is_sequential: true,
            allow_randomization: false,
            default_start_time: None,
            default_end_time: None,
            created_at: None,
        }
    }
}

/// Steps belonging to routines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineStep {
    pub id: String,
    pub routine_id: String,
    pub action_id: String,
    pub step_order: i64,
    pub min_duration_bucket: Option<i64>,
    pub max_duration_bucket: Option<i64>,
    pub created_at: Option<String>,
}

impl RoutineStep {
    pub fn new(
        routine_id: impl Into<String>,
        action_id: impl Into<String>,
        step_order: i64,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            routine_id: routine_id.into(),
            action_id: action_id.into(),
            step_order,
            min_duration_bucket: None,
            max_duration_bucket: None,
            created_at: None,
        }
    }
}
