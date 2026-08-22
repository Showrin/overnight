//! Generic child-process spawning and line-streaming, with zero knowledge of
//! any particular agent CLI. `providers/claude_code/` builds on top of this;
//! this module must never gain Claude-specific behavior.

pub mod json_lines;

use std::path::Path;
use std::pin::Pin;

use futures::Stream;
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("shell error: {0}")]
  Shell(#[from] tauri_plugin_shell::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A spawned child process together with a stream of its stdout lines.
/// Stderr is logged as it arrives rather than streamed — no current caller
/// needs to consume it programmatically.
pub struct SpawnedProcess {
  pub child: CommandChild,
  pub stdout_lines: Pin<Box<dyn Stream<Item = String> + Send>>,
}

pub fn spawn<R: Runtime>(
  app: &AppHandle<R>,
  program: &str,
  args: &[String],
  cwd: Option<&Path>,
) -> Result<SpawnedProcess> {
  let mut command = app.shell().command(program).args(args);
  if let Some(cwd) = cwd {
    command = command.current_dir(cwd);
  }
  let (rx, child) = command.spawn()?;

  Ok(SpawnedProcess {
    child,
    stdout_lines: stdout_lines_from_events(rx),
  })
}

/// Converts a raw `CommandEvent` receiver from `tauri-plugin-shell` into a
/// stream of stdout lines. Stderr/Error/Terminated events are logged, not
/// streamed — split out from `spawn` so this conversion logic is testable
/// without going through a full shell-plugin process spawn.
fn stdout_lines_from_events(
  mut rx: tokio::sync::mpsc::Receiver<CommandEvent>,
) -> Pin<Box<dyn Stream<Item = String> + Send>> {
  let (line_tx, line_rx) = tokio::sync::mpsc::channel::<String>(64);
  tauri::async_runtime::spawn(async move {
    while let Some(event) = rx.recv().await {
      match event {
        CommandEvent::Stdout(bytes) => {
          let line = String::from_utf8_lossy(&bytes).trim_end_matches(['\n', '\r']).to_string();
          if line_tx.send(line).await.is_err() {
            break;
          }
        }
        CommandEvent::Stderr(bytes) => {
          log::warn!("child process stderr: {}", String::from_utf8_lossy(&bytes).trim_end());
        }
        CommandEvent::Error(err) => {
          log::error!("child process error: {err}");
        }
        CommandEvent::Terminated(payload) => {
          log::info!("child process terminated: {payload:?}");
          break;
        }
        _ => {}
      }
    }
  });

  Box::pin(ReceiverStream::new(line_rx))
}

/// Emits a payload to every window in the app. Failures are logged, not
/// propagated — a webview that isn't listening yet shouldn't fail the
/// underlying agent session.
pub fn emit_to_webview<R: Runtime, S: serde::Serialize + Clone>(app: &AppHandle<R>, event: &str, payload: S) {
  if let Err(e) = app.emit(event, payload) {
    log::error!("failed to emit {event} to webview: {e}");
  }
}

#[cfg(test)]
mod tests {
  use futures::StreamExt;

  use super::*;

  #[tokio::test]
  async fn splits_stdout_into_lines() {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    tx.send(CommandEvent::Stdout(b"line one\n".to_vec())).await.unwrap();
    tx.send(CommandEvent::Stdout(b"line two\r\n".to_vec())).await.unwrap();
    drop(tx);

    let lines: Vec<String> = stdout_lines_from_events(rx).collect().await;
    assert_eq!(lines, vec!["line one".to_string(), "line two".to_string()]);
  }

  #[tokio::test]
  async fn stops_streaming_after_terminated() {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    tx.send(CommandEvent::Stdout(b"before\n".to_vec())).await.unwrap();
    tx.send(CommandEvent::Terminated(tauri_plugin_shell::process::TerminatedPayload {
      code: Some(0),
      signal: None,
    }))
    .await
    .unwrap();
    tx.send(CommandEvent::Stdout(b"after\n".to_vec())).await.unwrap();
    drop(tx);

    let lines: Vec<String> = stdout_lines_from_events(rx).collect().await;
    assert_eq!(lines, vec!["before".to_string()]);
  }

  #[tokio::test]
  async fn stderr_and_error_events_are_swallowed_not_streamed() {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    tx.send(CommandEvent::Stderr(b"warning\n".to_vec())).await.unwrap();
    tx.send(CommandEvent::Error("boom".to_string())).await.unwrap();
    tx.send(CommandEvent::Stdout(b"ok\n".to_vec())).await.unwrap();
    drop(tx);

    let lines: Vec<String> = stdout_lines_from_events(rx).collect().await;
    assert_eq!(lines, vec!["ok".to_string()]);
  }
}
