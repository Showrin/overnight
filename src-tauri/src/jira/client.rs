use serde::Deserialize;

use crate::jira::error::Result;
use crate::jira::wire::{Issue, SearchRequest, SearchResponse};

const SEARCH_FIELDS: [&str; 5] = ["summary", "status", "issuetype", "priority", "assignee"];
const PAGE_SIZE: u32 = 100;

#[derive(Deserialize)]
struct TenantInfo {
  #[serde(rename = "cloudId")]
  cloud_id: String,
}

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

  /// Scoped API tokens (the "API tokens with scopes" Atlassian now issues,
  /// e.g. with `read:jira-work`) are silently ignored by the site-direct
  /// domain: `{site}/rest/api/3/...` returns 200 with empty results instead
  /// of an auth error. They only work through the cloud gateway, which is
  /// addressed by cloud id rather than site hostname, so every API call
  /// needs to go through `api.atlassian.com/ex/jira/{cloud_id}` instead.
  /// The cloud id itself comes from this unauthenticated lookup.
  async fn resolve_cloud_id(&self) -> Result<String> {
    let info: TenantInfo = self
      .http
      .get(format!("{}/_edge/tenant_info", self.site))
      .send()
      .await?
      .error_for_status()?
      .json()
      .await?;
    Ok(info.cloud_id)
  }

  /// Runs `jql` against `/rest/api/3/search/jql`, following `nextPageToken`
  /// until Jira reports no more pages, and returns every issue normalized.
  pub async fn search_all(&self, jql: &str) -> Result<Vec<NormalizedIssue>> {
    let cloud_id = self.resolve_cloud_id().await?;
    let search_url = format!("https://api.atlassian.com/ex/jira/{cloud_id}/rest/api/3/search/jql");

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
        .post(&search_url)
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
