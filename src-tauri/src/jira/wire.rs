use serde::{Deserialize, Serialize};

// Wire-format structs for Jira Cloud REST v3's /rest/api/3/search/jql
// endpoint. Kept separate from the normalized shape the rest of the app
// consumes (see client::NormalizedIssue) so Jira's JSON quirks don't leak
// past this module.

#[derive(Serialize)]
pub struct SearchRequest<'a> {
  pub jql: &'a str,
  #[serde(rename = "maxResults")]
  pub max_results: u32,
  pub fields: Vec<&'a str>,
  #[serde(rename = "nextPageToken", skip_serializing_if = "Option::is_none")]
  pub next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
  pub issues: Vec<Issue>,
  #[serde(rename = "nextPageToken")]
  pub next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Issue {
  pub key: String,
  pub fields: IssueFields,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueFields {
  pub summary: String,
  pub status: StatusField,
  pub issuetype: Option<IssueTypeField>,
  pub priority: Option<PriorityField>,
  pub assignee: Option<AssigneeField>,
  #[serde(flatten)]
  pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StatusField {
  pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueTypeField {
  pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PriorityField {
  pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssigneeField {
  #[serde(rename = "displayName")]
  pub display_name: String,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_page_with_more_results() {
    let raw = r#"{
      "issues": [
        {
          "key": "OVN-17",
          "fields": {
            "summary": "1.3 Jira client + dashboard list",
            "status": {"name": "To Do"},
            "issuetype": {"name": "Subtask"},
            "priority": {"name": "Medium"},
            "assignee": null
          }
        }
      ],
      "nextPageToken": "CAEQAQ"
    }"#;

    let parsed: SearchResponse = serde_json::from_str(raw).unwrap();
    assert_eq!(parsed.issues.len(), 1);
    assert_eq!(parsed.issues[0].key, "OVN-17");
    assert_eq!(parsed.issues[0].fields.status.name, "To Do");
    assert!(parsed.issues[0].fields.assignee.is_none());
    assert_eq!(parsed.next_page_token.as_deref(), Some("CAEQAQ"));
  }

  #[test]
  fn parses_last_page() {
    let raw = r#"{
      "issues": [
        {
          "key": "OVN-18",
          "fields": {
            "summary": "1.4 Settings screen",
            "status": {"name": "To Do"},
            "issuetype": {"name": "Subtask"},
            "priority": {"name": "Medium"},
            "assignee": {"displayName": "Showrin Barua"}
          }
        }
      ],
      "nextPageToken": null
    }"#;

    let parsed: SearchResponse = serde_json::from_str(raw).unwrap();
    assert_eq!(parsed.issues.len(), 1);
    assert_eq!(
      parsed.issues[0].fields.assignee.as_ref().unwrap().display_name,
      "Showrin Barua"
    );
    assert!(parsed.next_page_token.is_none());
  }
}
