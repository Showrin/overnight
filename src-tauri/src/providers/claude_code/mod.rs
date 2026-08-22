//! `ClaudeCodeProvider`: the first (and for MVP, only) concrete
//! `AgentProvider`. All Claude Code-specific detail — CLI flags, the
//! `stream-json` wire format, `--resume` semantics — lives in this module
//! and must never leak into `db/` or `commands.rs`.

use std::path::Path;
use std::pin::Pin;
use std::sync::Mutex;

use futures::{Stream, StreamExt};
use tauri::AppHandle;

use crate::db::{self, DbPool};
use crate::providers::{AgentEvent, AgentProvider, Capability, Error, Plan, Result, SessionHandle, Usage};

const PROVIDER_NAME: &str = "claude_code";
const CLAUDE_BIN: &str = "claude";
const AGENT_EVENT_NAME: &str = "agent-event";

pub struct ClaudeCodeProvider;

impl ClaudeCodeProvider {
  fn spawn_session(
    &self,
    app: &AppHandle,
    pool: &DbPool,
    task_id: &str,
    parent_session_id: Option<&str>,
    prompt: &str,
    extra_args: &[String],
  ) -> Result<SessionHandle> {
    let conn = pool.get().map_err(db::error::Error::from)?;
    let task = db::tasks::get(&conn, task_id)?;
    let project = task.project_id.as_deref().map(|id| db::projects::get(&conn, id)).transpose()?;
    let repo_path = project.as_ref().map(|p| p.repo_path.as_str());

    let mut args = vec![
      "-p".to_string(),
      prompt.to_string(),
      "--output-format".to_string(),
      "stream-json".to_string(),
      "--permission-mode".to_string(),
      "plan".to_string(),
      "--verbose".to_string(),
    ];
    if let Some(path) = repo_path {
      args.push("--add-dir".to_string());
      args.push(path.to_string());
    }
    args.extend_from_slice(extra_args);

    let spawned = crate::process::spawn(app, CLAUDE_BIN, &args, repo_path.map(Path::new))?;
    let session = db::sessions::create(&conn, task_id, PROVIDER_NAME, "plan", parent_session_id)?;

    Ok(SessionHandle {
      session_id: session.id,
      child: Some(spawned.child),
      stdout_lines: Mutex::new(Some(spawned.stdout_lines)),
    })
  }
}

impl AgentProvider for ClaudeCodeProvider {
  fn launch_plan_session(&self, app: &AppHandle, pool: &DbPool, task_id: &str, prompt: &str) -> Result<SessionHandle> {
    self.spawn_session(app, pool, task_id, None, prompt, &[])
  }

  fn launch_autonomous_session(&self, app: &AppHandle, pool: &DbPool, task_id: &str, prompt: &str) -> Result<SessionHandle> {
    let conn = pool.get().map_err(db::error::Error::from)?;
    let task = db::tasks::get(&conn, task_id)?;
    let project_id = task
      .project_id
      .ok_or_else(|| Error::InvalidState("task has no project; can't find a sandbox to run in".to_string()))?;
    let sandbox = db::sandboxes::list_for_project(&conn, &project_id)?
      .into_iter()
      .find(|s| s.status == "running")
      .ok_or_else(|| Error::InvalidState("no running sandbox for this task's project".to_string()))?;
    let container_id = sandbox
      .container_id
      .clone()
      .ok_or_else(|| Error::InvalidState(format!("sandbox {} has no container", sandbox.id)))?;

    let args = vec![
      "exec".to_string(),
      "-w".to_string(),
      crate::docker::CONTAINER_WORKDIR.to_string(),
      container_id,
      CLAUDE_BIN.to_string(),
      "-p".to_string(),
      prompt.to_string(),
      "--output-format".to_string(),
      "stream-json".to_string(),
      "--permission-mode".to_string(),
      "bypassPermissions".to_string(),
      "--verbose".to_string(),
    ];

    let spawned = crate::process::spawn(app, "docker", &args, None)?;
    let session = db::sessions::create(&conn, task_id, PROVIDER_NAME, "autonomous", None)?;
    db::sessions::set_sandbox_id(&conn, &session.id, &sandbox.id)?;

    Ok(SessionHandle {
      session_id: session.id,
      child: Some(spawned.child),
      stdout_lines: Mutex::new(Some(spawned.stdout_lines)),
    })
  }

  fn stream_events(
    &self,
    app: &AppHandle,
    pool: &DbPool,
    handle: &SessionHandle,
  ) -> Pin<Box<dyn Stream<Item = AgentEvent> + Send>> {
    let raw_lines = match handle.stdout_lines.lock().unwrap().take() {
      Some(lines) => lines,
      None => return Box::pin(futures::stream::empty()),
    };

    let app = app.clone();
    let pool = pool.clone();
    let session_id = handle.session_id.clone();
    let json_values = crate::process::json_lines::parse(raw_lines);

    Box::pin(
      json_values
        .flat_map(|value| futures::stream::iter(translate_event(&value)))
        .map(move |event| {
          persist_and_emit(&app, &pool, &session_id, &event);
          event
        }),
    )
  }

  fn stop(&self, handle: &mut SessionHandle) -> Result<()> {
    if let Some(child) = handle.child.take() {
      child.kill().map_err(|e| Error::InvalidState(e.to_string()))?;
    }
    Ok(())
  }

  fn resume(
    &self,
    app: &AppHandle,
    pool: &DbPool,
    task_id: &str,
    parent_session_id: &str,
    provider_session_id: &str,
    prompt: &str,
  ) -> Result<SessionHandle> {
    self.spawn_session(
      app,
      pool,
      task_id,
      Some(parent_session_id),
      prompt,
      &["--resume".to_string(), provider_session_id.to_string()],
    )
  }

  fn capture_plan(&self, pool: &DbPool, handle: &SessionHandle) -> Result<Plan> {
    let conn = pool.get().map_err(db::error::Error::from)?;
    let events = db::activity::list_for_session(&conn, &handle.session_id)?;
    let content = events
      .iter()
      .filter(|e| e.event_type == "message")
      .filter_map(|e| serde_json::from_str::<serde_json::Value>(&e.payload).ok())
      .filter(|v| v.get("role").and_then(|r| r.as_str()) == Some("assistant"))
      .filter_map(|v| v.get("content").and_then(|c| c.as_str()).map(str::to_string))
      .collect::<Vec<_>>()
      .join("\n\n");
    Ok(Plan { content })
  }

  fn get_usage(&self, pool: &DbPool, handle: &SessionHandle) -> Result<Usage> {
    let conn = pool.get().map_err(db::error::Error::from)?;
    let (input_tokens, output_tokens) = db::metrics::total_tokens_for_session(&conn, &handle.session_id)?;
    let cost_usd = db::metrics::list_for_session(&conn, &handle.session_id)?
      .iter()
      .filter_map(|m| m.cost_usd)
      .reduce(|a, b| a + b);
    Ok(Usage { input_tokens, output_tokens, cost_usd })
  }

  fn supports(&self, capability: Capability) -> bool {
    matches!(capability, Capability::Resume | Capability::PlanCapture | Capability::UsageReporting)
  }
}

/// Translates one `stream-json` line into zero or more normalized events. A
/// single Claude Code line can carry several content blocks (e.g. text
/// followed by a tool call), so this returns a `Vec` rather than an
/// `Option`. Unrecognized event types/shapes are silently dropped — the
/// child-process layer already logs the underlying stderr/parse-error
/// signal, so no additional error path is needed here.
fn translate_event(value: &serde_json::Value) -> Vec<AgentEvent> {
  let Some(event_type) = value.get("type").and_then(|t| t.as_str()) else {
    return Vec::new();
  };

  match event_type {
    "system" => {
      if value.get("subtype").and_then(|s| s.as_str()) == Some("init") {
        if let Some(session_id) = value.get("session_id").and_then(|s| s.as_str()) {
          return vec![AgentEvent::SessionStarted { provider_session_id: session_id.to_string() }];
        }
      }
      Vec::new()
    }
    "assistant" => translate_message_blocks("assistant", value),
    "user" => translate_message_blocks("user", value),
    "result" => {
      let mut events = Vec::new();
      if let Some(usage) = value.get("usage") {
        events.push(AgentEvent::Usage {
          input_tokens: usage.get("input_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
          output_tokens: usage.get("output_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
          cost_usd: value.get("total_cost_usd").and_then(|v| v.as_f64()),
          model: value.get("model").and_then(|v| v.as_str()).map(str::to_string),
        });
      }
      let is_error = value.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
      events.push(AgentEvent::SessionEnded {
        status: if is_error { "failed" } else { "completed" }.to_string(),
      });
      events
    }
    _ => Vec::new(),
  }
}

fn translate_message_blocks(role: &str, value: &serde_json::Value) -> Vec<AgentEvent> {
  let Some(blocks) = value.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array()) else {
    return Vec::new();
  };

  blocks
    .iter()
    .filter_map(|block| match block.get("type").and_then(|t| t.as_str())? {
      "text" => Some(AgentEvent::Message {
        role: role.to_string(),
        content: block.get("text")?.as_str()?.to_string(),
      }),
      "tool_use" => Some(AgentEvent::ToolUse {
        id: block.get("id")?.as_str()?.to_string(),
        name: block.get("name")?.as_str()?.to_string(),
        input: block.get("input").cloned().unwrap_or(serde_json::Value::Null),
      }),
      "tool_result" => Some(AgentEvent::ToolResult {
        tool_use_id: block.get("tool_use_id")?.as_str()?.to_string(),
        output: block.get("content").cloned().unwrap_or(serde_json::Value::Null),
      }),
      _ => None,
    })
    .collect()
}

fn event_type_name(event: &AgentEvent) -> &'static str {
  match event {
    AgentEvent::SessionStarted { .. } => "session_started",
    AgentEvent::Message { .. } => "message",
    AgentEvent::ToolUse { .. } => "tool_use",
    AgentEvent::ToolResult { .. } => "tool_result",
    AgentEvent::Usage { .. } => "usage",
    AgentEvent::Error { .. } => "error",
    AgentEvent::SessionEnded { .. } => "session_ended",
  }
}

#[derive(Clone, serde::Serialize)]
struct EmittedEvent<'a> {
  session_id: &'a str,
  event: &'a AgentEvent,
}

/// Appends the event to `activity`, updates any derived db state
/// (`provider_session_id`, `metrics`), and emits it to the webview.
/// Db/emit failures are logged, not propagated — losing one event shouldn't
/// tear down the whole stream.
fn persist_and_emit(app: &AppHandle, pool: &DbPool, session_id: &str, event: &AgentEvent) {
  let conn = match pool.get() {
    Ok(conn) => conn,
    Err(e) => {
      log::error!("failed to get db connection to persist agent event: {e}");
      crate::process::emit_to_webview(app, AGENT_EVENT_NAME, EmittedEvent { session_id, event });
      return;
    }
  };

  let payload = serde_json::to_string(event).unwrap_or_default();
  if let Err(e) = db::activity::append(&conn, session_id, event_type_name(event), &payload) {
    log::error!("failed to append activity for session {session_id}: {e}");
  }

  match event {
    AgentEvent::SessionStarted { provider_session_id } => {
      if let Err(e) = db::sessions::set_provider_session_id(&conn, session_id, provider_session_id) {
        log::error!("failed to persist provider_session_id for session {session_id}: {e}");
      }
    }
    AgentEvent::Usage { input_tokens, output_tokens, cost_usd, model } => {
      if let Err(e) = db::metrics::record(&conn, session_id, *input_tokens, *output_tokens, *cost_usd, model.as_deref())
      {
        log::error!("failed to record metrics for session {session_id}: {e}");
      }
    }
    _ => {}
  }

  crate::process::emit_to_webview(app, AGENT_EVENT_NAME, EmittedEvent { session_id, event });
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn translates_init_event_to_session_started() {
    let value = json!({"type": "system", "subtype": "init", "session_id": "abc-123"});
    assert_eq!(
      translate_event(&value),
      vec![AgentEvent::SessionStarted { provider_session_id: "abc-123".to_string() }]
    );
  }

  #[test]
  fn translates_assistant_text_and_tool_use_blocks() {
    let value = json!({
      "type": "assistant",
      "message": {
        "role": "assistant",
        "content": [
          {"type": "text", "text": "Let me check that file."},
          {"type": "tool_use", "id": "tu_1", "name": "Read", "input": {"file_path": "foo.rs"}},
        ]
      }
    });

    assert_eq!(
      translate_event(&value),
      vec![
        AgentEvent::Message { role: "assistant".to_string(), content: "Let me check that file.".to_string() },
        AgentEvent::ToolUse {
          id: "tu_1".to_string(),
          name: "Read".to_string(),
          input: json!({"file_path": "foo.rs"}),
        },
      ]
    );
  }

  #[test]
  fn translates_user_tool_result_block() {
    let value = json!({
      "type": "user",
      "message": {
        "role": "user",
        "content": [
          {"type": "tool_result", "tool_use_id": "tu_1", "content": "file contents"},
        ]
      }
    });

    assert_eq!(
      translate_event(&value),
      vec![AgentEvent::ToolResult { tool_use_id: "tu_1".to_string(), output: json!("file contents") }]
    );
  }

  #[test]
  fn translates_result_event_to_usage_and_session_ended() {
    let value = json!({
      "type": "result",
      "is_error": false,
      "model": "claude-sonnet-5",
      "total_cost_usd": 0.05,
      "usage": {"input_tokens": 100, "output_tokens": 50}
    });

    assert_eq!(
      translate_event(&value),
      vec![
        AgentEvent::Usage {
          input_tokens: 100,
          output_tokens: 50,
          cost_usd: Some(0.05),
          model: Some("claude-sonnet-5".to_string()),
        },
        AgentEvent::SessionEnded { status: "completed".to_string() },
      ]
    );
  }

  #[test]
  fn translates_errored_result_event_to_failed_status() {
    let value = json!({"type": "result", "is_error": true});
    assert_eq!(translate_event(&value), vec![AgentEvent::SessionEnded { status: "failed".to_string() }]);
  }

  #[test]
  fn unrecognized_event_type_is_dropped() {
    let value = json!({"type": "something_new_and_unknown"});
    assert!(translate_event(&value).is_empty());
  }

  #[test]
  fn missing_type_field_is_dropped() {
    let value = json!({"no_type_here": true});
    assert!(translate_event(&value).is_empty());
  }
}
