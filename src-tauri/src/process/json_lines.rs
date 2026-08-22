//! Parses a stream of raw text lines as JSON-lines (one `serde_json::Value`
//! per line). Malformed lines are logged and skipped rather than failing the
//! whole stream — a single garbled line from a child process shouldn't take
//! down the rest of the session.

use std::pin::Pin;

use futures::{Stream, StreamExt};
use serde_json::Value;

pub fn parse<S>(lines: S) -> Pin<Box<dyn Stream<Item = Value> + Send>>
where
  S: Stream<Item = String> + Send + 'static,
{
  Box::pin(lines.filter_map(|line| async move {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      return None;
    }
    match serde_json::from_str::<Value>(trimmed) {
      Ok(value) => Some(value),
      Err(e) => {
        log::warn!("skipping malformed stream-json line: {e}");
        None
      }
    }
  }))
}

#[cfg(test)]
mod tests {
  use futures::StreamExt;
  use serde_json::json;

  use super::*;

  #[tokio::test]
  async fn parses_valid_lines_and_skips_malformed_and_empty() {
    let lines = futures::stream::iter(vec![
      r#"{"type":"init","session_id":"abc"}"#.to_string(),
      "".to_string(),
      "not json at all".to_string(),
      r#"{"type":"result","ok":true}"#.to_string(),
      "   ".to_string(),
    ]);

    let values: Vec<Value> = parse(lines).collect().await;

    assert_eq!(
      values,
      vec![
        json!({"type": "init", "session_id": "abc"}),
        json!({"type": "result", "ok": true}),
      ]
    );
  }
}
