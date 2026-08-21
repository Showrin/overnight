use keyring::Entry;

use crate::jira::error::{Error, Result};

const SERVICE: &str = "overnight";
const ACCOUNT: &str = "jira_api_token";

pub fn store_token(token: &str) -> Result<()> {
  Entry::new(SERVICE, ACCOUNT)?.set_password(token)?;
  Ok(())
}

pub fn get_token() -> Result<String> {
  Entry::new(SERVICE, ACCOUNT)?
    .get_password()
    .map_err(|e| match e {
      keyring::Error::NoEntry => Error::NoToken,
      other => Error::Keyring(other),
    })
}

pub fn has_token() -> bool {
  get_token().is_ok()
}
