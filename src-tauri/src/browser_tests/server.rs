//! Loopback HTTP API the sandbox's `overnight-browser-test` helper talks to.
//! Every request carries `Authorization: Bearer <sandbox chrome_token>`;
//! the token identifies the sandbox, and requests from a sandbox with
//! Claude in Chrome turned off are refused.

use std::net::SocketAddr;

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use super::{Hub, SERVER_PORT};
use crate::db::models::{BrowserTest, Sandbox};
use crate::db::{browser_tests, sandboxes};

const MAX_BODY_BYTES: usize = 512 * 1024;

pub fn router(hub: Hub) -> Router {
  Router::new()
    .route("/v1/health", get(health))
    .route("/v1/browser-tests", post(create_test))
    .route("/v1/browser-tests/{id}", get(get_test))
    .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
    .with_state(hub)
}

/// Binds IPv4 and IPv6 loopback — the sbx proxy delivers
/// `host.docker.internal` traffic to "localhost", which may resolve to
/// either. Only failing both is fatal.
pub async fn serve(hub: Hub) -> Result<(), String> {
  let app = router(hub);
  let mut bound = Vec::new();
  let mut errors = Vec::new();
  for addr in [SocketAddr::from(([127, 0, 0, 1], SERVER_PORT)), SocketAddr::from(([0u16, 0, 0, 0, 0, 0, 0, 1], SERVER_PORT))] {
    match tokio::net::TcpListener::bind(addr).await {
      Ok(listener) => bound.push(listener),
      Err(e) => errors.push(format!("{addr}: {e}")),
    }
  }
  if bound.is_empty() {
    return Err(format!("couldn't listen for browser tests: {}", errors.join("; ")));
  }
  for e in &errors {
    log::warn!("browser test server: {e}");
  }
  let serves = bound.into_iter().map(|listener| {
    let app = app.clone();
    async move { axum::serve(listener, app).await }
  });
  for result in futures::future::join_all(serves).await {
    result.map_err(|e| e.to_string())?;
  }
  Ok(())
}

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
  }
}

fn internal(e: impl std::fmt::Display) -> ApiError {
  ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn authorize(hub: &Hub, headers: &HeaderMap) -> Result<Sandbox, ApiError> {
  let token = headers
    .get("authorization")
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.strip_prefix("Bearer "))
    .map(str::trim)
    .filter(|t| !t.is_empty())
    .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "missing bearer token (OVERNIGHT_BROWSER_TOKEN)".to_string()))?;
  let conn = hub.pool.get().map_err(internal)?;
  let sandbox = sandboxes::find_by_chrome_token(&conn, token)
    .map_err(internal)?
    .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "unknown token".to_string()))?;
  if !sandbox.chrome_enabled {
    return Err(ApiError(
      StatusCode::FORBIDDEN,
      "Claude in Chrome is turned off for this sandbox — enable it in Overnight's Browser tab".to_string(),
    ));
  }
  Ok(sandbox)
}

#[derive(Serialize)]
struct Health {
  ok: bool,
  target: String,
}

async fn health(State(hub): State<Hub>, headers: HeaderMap) -> Result<Json<Health>, ApiError> {
  let sandbox = authorize(&hub, &headers)?;
  Ok(Json(Health { ok: true, target: sandbox.chrome_target }))
}

#[derive(Deserialize)]
struct CreateRequest {
  doc: String,
  branch: Option<String>,
  port: Option<i64>,
}

/// What the sandbox sees of a test — no sandbox ids or host paths.
#[derive(Serialize)]
struct TestView {
  id: String,
  status: String,
  verdict: Option<String>,
  report: Option<String>,
  error: Option<String>,
}

impl From<BrowserTest> for TestView {
  fn from(t: BrowserTest) -> Self {
    Self { id: t.id, status: t.status, verdict: t.verdict, report: t.report, error: t.error }
  }
}

async fn create_test(
  State(hub): State<Hub>,
  headers: HeaderMap,
  Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<TestView>), ApiError> {
  let sandbox = authorize(&hub, &headers)?;
  if let Some(port) = body.port {
    if !(1..=65535).contains(&port) {
      return Err(ApiError(StatusCode::BAD_REQUEST, format!("port out of range: {port}")));
    }
  }
  let status = if sandbox.chrome_target == "external" { "awaiting_host" } else { "queued" };
  let branch = body.branch.as_deref().map(str::trim).filter(|b| !b.is_empty());
  let conn = hub.pool.get().map_err(internal)?;
  let test = browser_tests::create(&conn, &sandbox.id, &body.doc, branch, body.port, status)
    .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
  hub.changed(&test);
  Ok((StatusCode::CREATED, Json(test.into())))
}

async fn get_test(State(hub): State<Hub>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<TestView>, ApiError> {
  let sandbox = authorize(&hub, &headers)?;
  let conn = hub.pool.get().map_err(internal)?;
  match browser_tests::get(&conn, &id) {
    Ok(test) if test.sandbox_id == sandbox.id => Ok(Json(test.into())),
    Ok(_) | Err(crate::db::error::Error::NotFound) => Err(ApiError(StatusCode::NOT_FOUND, "no such test".to_string())),
    Err(e) => Err(internal(e)),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use axum::body::Body;
  use axum::http::Request;
  use r2d2_sqlite::SqliteConnectionManager;
  use std::sync::{Arc, Mutex};
  use tower::ServiceExt;

  struct Fixture {
    hub: Hub,
    changes: Arc<Mutex<Vec<String>>>,
    sandbox_id: String,
    token: String,
  }

  fn fixture(target: &str) -> Fixture {
    // A shared-cache in-memory DB so every pooled connection sees the same data.
    let uri = format!("file:bt-{}?mode=memory&cache=shared", uuid::Uuid::new_v4().simple());
    let manager = SqliteConnectionManager::file(uri).with_flags(
      rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    );
    let pool = r2d2::Pool::builder().max_size(2).build(manager).unwrap();
    let mut conn = pool.get().unwrap();
    crate::db::migrations::migrations().to_latest(&mut conn).unwrap();
    let project = crate::db::projects::create(&conn, "P", "/repo", None, None).unwrap();
    let sandbox = sandboxes::create(&conn, &project.id, "mount", None, None, "default", None, "claude").unwrap();
    let url = (target == "external").then_some("http://localhost:3000");
    let sandbox = sandboxes::set_chrome_settings(&conn, &sandbox.id, true, target, None, url).unwrap();
    drop(conn);
    let changes = Arc::new(Mutex::new(Vec::new()));
    let sink = changes.clone();
    let hub = Hub::new(pool, move |t| sink.lock().unwrap().push(t.status.clone()));
    Fixture { hub, changes, sandbox_id: sandbox.id, token: sandbox.chrome_token.unwrap() }
  }

  async fn call(hub: &Hub, method: &str, uri: &str, token: Option<&str>, body: Option<serde_json::Value>) -> (StatusCode, serde_json::Value) {
    let mut req = Request::builder().method(method).uri(uri).header("content-type", "application/json");
    if let Some(token) = token {
      req = req.header("authorization", format!("Bearer {token}"));
    }
    let body = body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty);
    let res = router(hub.clone()).oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
  }

  #[tokio::test]
  async fn rejects_missing_unknown_and_disabled_tokens() {
    let f = fixture("sandbox");
    assert_eq!(call(&f.hub, "GET", "/v1/health", None, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&f.hub, "GET", "/v1/health", Some("nope"), None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&f.hub, "GET", "/v1/health", Some(&f.token), None).await.0, StatusCode::OK);

    let conn = f.hub.pool.get().unwrap();
    sandboxes::set_chrome_settings(&conn, &f.sandbox_id, false, "sandbox", None, None).unwrap();
    drop(conn);
    assert_eq!(call(&f.hub, "GET", "/v1/health", Some(&f.token), None).await.0, StatusCode::FORBIDDEN);
  }

  #[tokio::test]
  async fn sandbox_target_queues_and_returns_the_test() {
    let f = fixture("sandbox");
    let (status, body) =
      call(&f.hub, "POST", "/v1/browser-tests", Some(&f.token), Some(serde_json::json!({ "doc": "Open /login", "port": 5173 }))).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["status"], "queued");
    assert_eq!(*f.changes.lock().unwrap(), vec!["queued".to_string()]);

    let id = body["id"].as_str().unwrap();
    let (status, body) = call(&f.hub, "GET", &format!("/v1/browser-tests/{id}"), Some(&f.token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "queued");
  }

  #[tokio::test]
  async fn external_target_waits_for_the_host() {
    let f = fixture("external");
    let (_, body) =
      call(&f.hub, "POST", "/v1/browser-tests", Some(&f.token), Some(serde_json::json!({ "doc": "x", "branch": "feat/a" }))).await;
    assert_eq!(body["status"], "awaiting_host");
  }

  #[tokio::test]
  async fn rejects_bad_requests_and_other_sandboxes_tests() {
    let f = fixture("sandbox");
    let empty = call(&f.hub, "POST", "/v1/browser-tests", Some(&f.token), Some(serde_json::json!({ "doc": " " }))).await;
    assert_eq!(empty.0, StatusCode::BAD_REQUEST);
    let bad_port = call(&f.hub, "POST", "/v1/browser-tests", Some(&f.token), Some(serde_json::json!({ "doc": "x", "port": 0 }))).await;
    assert_eq!(bad_port.0, StatusCode::BAD_REQUEST);

    let conn = f.hub.pool.get().unwrap();
    let project = crate::db::projects::create(&conn, "Q", "/repo2", None, None).unwrap();
    let other = sandboxes::create(&conn, &project.id, "clone", None, None, "default", None, "claude").unwrap();
    let foreign = browser_tests::create(&conn, &other.id, "x", None, None, "queued").unwrap();
    drop(conn);
    let (status, _) = call(&f.hub, "GET", &format!("/v1/browser-tests/{}", foreign.id), Some(&f.token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
  }
}
