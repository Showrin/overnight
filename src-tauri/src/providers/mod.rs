//! Ports-and-adapters abstraction over "coding agent backend" (Claude Code
//! first, others later). Nothing outside `providers/claude_code/` may depend
//! on a specific agent CLI's flags, wire format, or session-file
//! conventions — see ARCHITECTURE.md's "AgentProvider abstraction" section.

pub mod claude_code;

use std::pin::Pin;
use std::sync::Mutex;

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
  // Constructed by `stop()`/`launch_autonomous_session()`, which no caller
  // reaches yet — see the trait-level dead_code note below.
  #[allow(dead_code)]
  #[error("not implemented: {0}")]
  NotImplemented(&'static str),
  #[allow(dead_code)]
  #[error("invalid state: {0}")]
  InvalidState(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A capability an `AgentProvider` may or may not support. Lets the UI adapt
/// per-provider instead of assuming feature parity — see the "Non-goal"
/// note in ARCHITECTURE.md. Only capabilities an existing trait method
/// already exposes belong here; this is not a place to speculate about
/// features no code path uses yet.
// Only `ClaudeCodeProvider::supports()` constructs/matches these today — a
// real caller (capability-aware UI) lands with the chat UI ticket.
#[allow(dead_code)]
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
  // Reserved for translation-layer failures (e.g. a stream-json line that
  // parses but doesn't match any known shape); no such case has arisen yet.
  #[allow(dead_code)]
  Error { message: String },
  SessionEnded { status: String },
}

// Return types of `capture_plan`/`get_usage`, which no caller reaches yet
// (no chat UI to show a captured plan or usage numbers) — see the
// trait-level dead_code note below.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Plan {
  pub content: String,
}

#[allow(dead_code)]
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
  /// `None` once `stop()` has taken and killed it. Not yet read anywhere
  /// but by `stop()` itself, which no caller reaches yet (see below).
  #[allow(dead_code)]
  pub(crate) child: Option<tauri_plugin_shell::process::CommandChild>,
  /// Taken (via `Mutex::lock().take()`) the first time `stream_events` is
  /// called — a process's stdout can only be consumed once.
  pub(crate) stdout_lines: Mutex<Option<Pin<Box<dyn Stream<Item = String> + Send>>>>,
}

pub trait AgentProvider: Send + Sync {
  fn launch_plan_session(&self, app: &AppHandle, pool: &DbPool, task_id: &str, prompt: &str) -> Result<SessionHandle>;

  // The methods below aren't reachable from `commands.rs` yet — there's no
  // chat UI to trigger stop/resume, show a captured plan, display usage,
  // or adapt to per-provider capabilities. They're implemented and unit
  // tested (see `providers::claude_code`) against ARCHITECTURE.md's target
  // trait shape now, so that ticket can wire them up directly instead of
  // re-deriving this layer.
  #[allow(dead_code)]
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

  #[allow(dead_code)]
  fn stop(&self, handle: &mut SessionHandle) -> Result<()>;

  #[allow(dead_code, clippy::too_many_arguments)]
  fn resume(
    &self,
    app: &AppHandle,
    pool: &DbPool,
    task_id: &str,
    parent_session_id: &str,
    provider_session_id: &str,
    prompt: &str,
  ) -> Result<SessionHandle>;

  #[allow(dead_code)]
  fn capture_plan(&self, pool: &DbPool, handle: &SessionHandle) -> Result<Plan>;

  #[allow(dead_code)]
  fn get_usage(&self, pool: &DbPool, handle: &SessionHandle) -> Result<Usage>;

  #[allow(dead_code)]
  fn supports(&self, capability: Capability) -> bool;
}
