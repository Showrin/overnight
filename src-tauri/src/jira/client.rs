use crate::jira::error::Result;
use crate::jira::wire::{Issue, SearchRequest, SearchResponse};

const SEARCH_FIELDS: [&str; 5] = ["summary", "status", "issuetype", "priority", "assignee"];
const PAGE_SIZE: u32 = 100;

/// A Jira issue normalized into the shape the rest of the app (and the
/// SQLite cache) consumes, decoupled from Jira's wire format.
pub struct NormalizedIssue {
  pub key: String,
  pub summary: String,
  pub status: String,
  pub issue_type: Option<String>,
  pub priority: Option<String>,
  pub assignee: Option<String>,
  pub url: String,
  pub raw_fields: String,
}

pub struct JiraClient {
  http: reqwest::Client,
  site: String,
  email: String,
  token: String,
}

impl JiraClient {
  pub fn new(site: String, email: String, token: String) -> Self {
    Self {
      http: reqwest::Client::new(),
      site: site.trim_end_matches('/').to_string(),
      email,
      token,
    }
  }

  /// Runs `jql` against `/rest/api/3/search/jql`, following `nextPageToken`
  /// until Jira reports no more pages, and returns every issue normalized.
  pub async fn search_all(&self, jql: &str) -> Result<Vec<NormalizedIssue>> {
    let mut out = Vec::new();
    let mut next_page_token: Option<String> = None;

    loop {
      let body = SearchRequest {
        jql,
        max_results: PAGE_SIZE,
        fields: SEARCH_FIELDS.to_vec(),
        next_page_token: next_page_token.clone(),
      };

      let response: SearchResponse = self
        .http
        .post(format!("{}/rest/api/3/search/jql", self.site))
        .basic_auth(&self.email, Some(&self.token))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

      if response.issues.is_empty() {
        break;
      }

      out.extend(response.issues.into_iter().map(|issue| self.normalize(issue)));

      match response.next_page_token {
        Some(token) => next_page_token = Some(token),
        None => break,
      }
    }

    Ok(out)
  }

  fn normalize(&self, issue: Issue) -> NormalizedIssue {
    let key = issue.key;
    let fields = issue.fields;
    let raw_fields = serde_json::to_string(&fields).unwrap_or_default();
    NormalizedIssue {
      url: format!("{}/browse/{}", self.site, key),
      key,
      summary: fields.summary,
      status: fields.status.name,
      issue_type: fields.issuetype.map(|t| t.name),
      priority: fields.priority.map(|p| p.name),
      assignee: fields.assignee.map(|a| a.display_name),
      raw_fields,
    }
  }
}
