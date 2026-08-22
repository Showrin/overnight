//! Ports-and-adapters abstraction over "coding agent backend" (Claude Code
//! first, others later). Nothing outside `providers/claude_code/` may depend
//! on a specific agent CLI's flags, wire format, or session-file
//! conventions — see ARCHITECTURE.md's "AgentProvider abstraction" section.

use std::pin::Pin;

use futures::Stream;
use serde::Serialize;
use tauri::AppHandle;

use crate::db::DbPool;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("process error: {0}")]
  Process(#[from] crate::process::Error),
  #[error("db error: {0}")]
  Db(#[from] crate::db::error::Error),
  #[error("not implemented: {0}")]
  NotImplemented(&'static str),
  #[error("invalid state: {0}")]
  InvalidState(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A capability an `AgentProvider` may or may not support. Lets the UI adapt
/// per-provider instead of assuming feature parity — see the "Non-goal"
/// note in ARCHITECTURE.md. Only capabilities an existing trait method
/// already exposes belong here; this is not a place to speculate about
/// features no code path uses yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
  Resume,
  PlanCapture,
  UsageReporting,
}

/// A normalized agent event. Providers translate their own wire format
/// (e.g. Claude Code's `stream-json`) into this before it ever reaches the
/// frontend or the `activity` table.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
  SessionStarted { provider_session_id: String },
  Message { role: String, content: String },
  ToolUse { id: String, name: String, input: serde_json::Value },
  ToolResult { tool_use_id: String, output: serde_json::Value },
  Usage { input_tokens: i64, output_tokens: i64, cost_usd: Option<f64>, model: Option<String> },
  Error { message: String },
  SessionEnded { status: String },
}

#[derive(Debug, Clone)]
pub struct Plan {
  pub content: String,
}

#[derive(Debug, Clone)]
pub struct Usage {
  pub input_tokens: i64,
  pub output_tokens: i64,
  pub cost_usd: Option<f64>,
}

/// Opaque handle to a running (or resumed) agent session. Callers only ever
/// pass this back to the same `AgentProvider` that created it; what's
/// inside is provider-specific.
pub struct SessionHandle {
  /// `sessions.id` (our db row), not the provider's own session id.
  pub session_id: String,
  pub(crate) child: tauri_plugin_shell::process::CommandChild,
}

pub trait AgentProvider: Send + Sync {
  fn launch_plan_session(&self, app: &AppHandle, pool: &DbPool, task_id: &str, prompt: &str) -> Result<SessionHandle>;

  fn launch_autonomous_session(
    &self,
    app: &AppHandle,
    pool: &DbPool,
    task_id: &str,
    prompt: &str,
  ) -> Result<SessionHandle>;

  fn stream_events(
    &self,
    app: &AppHandle,
    pool: &DbPool,
    handle: &SessionHandle,
  ) -> Pin<Box<dyn Stream<Item = AgentEvent> + Send>>;

  fn stop(&self, handle: &mut SessionHandle) -> Result<()>;

  fn resume(&self, app: &AppHandle, pool: &DbPool, provider_session_id: &str, prompt: &str) -> Result<SessionHandle>;

  fn capture_plan(&self, pool: &DbPool, handle: &SessionHandle) -> Result<Plan>;

  fn get_usage(&self, pool: &DbPool, handle: &SessionHandle) -> Result<Usage>;

  fn supports(&self, capability: Capability) -> bool;
}
