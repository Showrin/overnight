use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("database error: {0}")]
  Sqlite(#[from] rusqlite::Error),
  #[error("connection pool error: {0}")]
  Pool(#[from] r2d2::Error),
  #[error("migration error: {0}")]
  Migration(#[from] rusqlite_migration::Error),
  #[error("json error: {0}")]
  Json(#[from] serde_json::Error),
  #[error("not found")]
  NotFound,
  #[error("not a git repository: {0}")]
  InvalidRepoPath(String),
  #[error("invalid value: {0}")]
  InvalidValue(String),
}

// Tauri commands require their Err type to implement Serialize to cross IPC.
// We stringify rather than trying to preserve structure, since the frontend
// only needs a human-readable message.
impl Serialize for Error {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(&self.to_string())
  }
}

pub type Result<T> = std::result::Result<T, Error>;
