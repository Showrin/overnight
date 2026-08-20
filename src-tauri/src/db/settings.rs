use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};

use crate::db::error::{Error, Result};
use crate::db::models::now_millis;

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
  conn
    .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0))
    .map(Some)
    .or_else(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => Ok(None),
      other => Err(Error::Sqlite(other)),
    })
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
  let now = now_millis();
  conn.execute(
    "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    params![key, value, now],
  )?;
  Ok(())
}

pub fn get_json<T: DeserializeOwned>(conn: &Connection, key: &str) -> Result<Option<T>> {
  match get(conn, key)? {
    Some(raw) => Ok(Some(serde_json::from_str(&raw)?)),
    None => Ok(None),
  }
}

pub fn set_json<T: Serialize>(conn: &Connection, key: &str, value: &T) -> Result<()> {
  let raw = serde_json::to_string(value)?;
  set(conn, key, &raw)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;
  use serde::Deserialize;

  #[derive(Debug, Serialize, Deserialize, PartialEq)]
  struct Prefs {
    theme: String,
    idle_timeout_minutes: u32,
  }

  #[test]
  fn get_missing_returns_none() {
    let conn = test_conn();
    assert_eq!(get(&conn, "missing").unwrap(), None);
    assert_eq!(get_json::<Prefs>(&conn, "missing").unwrap(), None);
  }

  #[test]
  fn set_then_get_roundtrips_and_upserts() {
    let conn = test_conn();
    set(&conn, "theme", "dark").unwrap();
    assert_eq!(get(&conn, "theme").unwrap().as_deref(), Some("dark"));

    set(&conn, "theme", "light").unwrap();
    assert_eq!(get(&conn, "theme").unwrap().as_deref(), Some("light"));
  }

  #[test]
  fn json_roundtrip() {
    let conn = test_conn();
    let prefs = Prefs {
      theme: "dark".to_string(),
      idle_timeout_minutes: 30,
    };
    set_json(&conn, "prefs", &prefs).unwrap();
    let fetched: Prefs = get_json(&conn, "prefs").unwrap().unwrap();
    assert_eq!(fetched, prefs);
  }
}
