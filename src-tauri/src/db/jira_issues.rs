use rusqlite::{params, Connection};

use crate::db::error::Result;
use crate::db::models::{new_id, now_millis, JiraIssue};
use crate::jira::NormalizedIssue;

fn row_to_issue(row: &rusqlite::Row) -> rusqlite::Result<JiraIssue> {
  Ok(JiraIssue {
    id: row.get("id")?,
    key: row.get("key")?,
    summary: row.get("summary")?,
    status: row.get("status")?,
    issue_type: row.get("issue_type")?,
    priority: row.get("priority")?,
    assignee: row.get("assignee")?,
    url: row.get("url")?,
    raw_fields: row.get("raw_fields")?,
    synced_at: row.get("synced_at")?,
    created_at: row.get("created_at")?,
    updated_at: row.get("updated_at")?,
  })
}

/// Upserts a batch of issues fetched from a Jira sync, keyed on `key`. Takes
/// `jira::NormalizedIssue` directly rather than a separate DTO to avoid
/// duplicating an almost-identical struct across the db/jira boundary.
pub fn upsert_many(conn: &mut Connection, issues: &[NormalizedIssue]) -> Result<usize> {
  let now = now_millis();
  let tx = conn.transaction()?;
  for issue in issues {
    tx.execute(
      "INSERT INTO jira_issues
         (id, key, summary, status, issue_type, priority, assignee, url, raw_fields, synced_at, created_at, updated_at)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
       ON CONFLICT(key) DO UPDATE SET
         summary = excluded.summary,
         status = excluded.status,
         issue_type = excluded.issue_type,
         priority = excluded.priority,
         assignee = excluded.assignee,
         url = excluded.url,
         raw_fields = excluded.raw_fields,
         synced_at = excluded.synced_at,
         updated_at = excluded.updated_at",
      params![
        new_id(),
        issue.key,
        issue.summary,
        issue.status,
        issue.issue_type,
        issue.priority,
        issue.assignee,
        issue.url,
        issue.raw_fields,
        now,
        now,
        now,
      ],
    )?;
  }
  tx.commit()?;
  Ok(issues.len())
}

pub fn list(conn: &Connection) -> Result<Vec<JiraIssue>> {
  let mut stmt = conn.prepare("SELECT * FROM jira_issues ORDER BY updated_at DESC")?;
  let rows = stmt.query_map([], row_to_issue)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;

  fn issue(key: &str, summary: &str) -> NormalizedIssue {
    NormalizedIssue {
      key: key.to_string(),
      summary: summary.to_string(),
      status: "To Do".to_string(),
      issue_type: Some("Subtask".to_string()),
      priority: Some("Medium".to_string()),
      assignee: None,
      url: format!("https://example.atlassian.net/browse/{key}"),
      raw_fields: "{}".to_string(),
    }
  }

  #[test]
  fn upsert_then_list() {
    let mut conn = test_conn();
    upsert_many(&mut conn, &[issue("OVN-17", "Jira client")]).unwrap();

    let all = list(&conn).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].key, "OVN-17");
    assert_eq!(all[0].summary, "Jira client");
  }

  #[test]
  fn resync_is_idempotent_and_updates_in_place() {
    let mut conn = test_conn();
    upsert_many(&mut conn, &[issue("OVN-17", "Jira client")]).unwrap();
    upsert_many(&mut conn, &[issue("OVN-17", "Jira client + dashboard list")]).unwrap();

    let all = list(&conn).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].summary, "Jira client + dashboard list");
  }
}
