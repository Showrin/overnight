use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("jira request failed: {0}")]
  Http(#[from] reqwest::Error),
  #[error("keychain error: {0}")]
  Keyring(#[from] keyring::Error),
  #[error("no Jira API token configured")]
  NoToken,
  #[error("jira config incomplete: {0}")]
  ConfigMissing(&'static str),
}

// Same rationale as db::error::Error: Tauri command errors must cross IPC as
// JSON, and the frontend only needs a human-readable message.
impl Serialize for Error {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(&self.to_string())
  }
}

pub type Result<T> = std::result::Result<T, Error>;
