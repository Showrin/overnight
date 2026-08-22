use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
  pub id: String,
  pub title: String,
  pub description: Option<String>,
  pub jira_key: Option<String>,
  pub status: String,
  pub project_path: Option<String>,
  pub project_id: Option<String>,
  pub metadata: Option<String>,
  pub created_at: i64,
  pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
  pub id: String,
  pub task_id: String,
  pub agent_provider: String,
  pub provider_session_id: Option<String>,
  pub parent_session_id: Option<String>,
  pub mode: String,
  pub status: String,
  pub started_at: i64,
  pub ended_at: Option<i64>,
  pub transcript_path: Option<String>,
  pub plan_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
  pub id: String,
  pub session_id: String,
  pub occurred_at: i64,
  pub event_type: String,
  pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
  pub id: String,
  pub session_id: String,
  pub captured_at: i64,
  pub tokens_input: i64,
  pub tokens_output: i64,
  pub cost_usd: Option<f64>,
  pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerMetric {
  pub id: String,
  pub session_id: String,
  pub captured_at: i64,
  pub cpu_percent: f64,
  pub memory_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraIssue {
  pub id: String,
  pub key: String,
  pub summary: String,
  pub status: String,
  pub issue_type: Option<String>,
  pub priority: Option<String>,
  pub assignee: Option<String>,
  pub url: String,
  pub raw_fields: String,
  pub synced_at: i64,
  pub created_at: i64,
  pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
  pub id: String,
  pub name: String,
  pub repo_path: String,
  pub plans_path: Option<String>,
  pub dev_server_port: Option<i64>,
  pub extra_clone_paths: Vec<String>,
  pub created_at: i64,
  pub updated_at: i64,
}

#[allow(dead_code)] // not yet exposed via commands.rs; settings::get/set work directly with raw values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
  pub key: String,
  pub value: String,
  pub updated_at: i64,
}

pub fn now_millis() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_millis() as i64
}

pub fn new_id() -> String {
  uuid::Uuid::new_v4().to_string()
}
