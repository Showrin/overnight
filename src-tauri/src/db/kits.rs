use rusqlite::{params, Connection};

use crate::db::error::{Error, Result};
use crate::db::models::{new_id, now_millis, Kit};

fn project_ids(conn: &Connection, kit_id: &str) -> Result<Vec<String>> {
  let mut stmt = conn.prepare("SELECT id FROM projects WHERE kit_id = ?1 ORDER BY name")?;
  let rows = stmt.query_map(params![kit_id], |row| row.get(0))?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn row_to_kit(row: &rusqlite::Row) -> rusqlite::Result<Kit> {
  Ok(Kit {
    id: row.get("id")?,
    name: row.get("name")?,
    spec: row.get("spec")?,
    is_global: row.get::<_, i64>("is_global")? != 0,
    project_ids: Vec::new(),
    created_at: row.get("created_at")?,
    updated_at: row.get("updated_at")?,
  })
}

fn with_projects(conn: &Connection, mut kit: Kit) -> Result<Kit> {
  kit.project_ids = project_ids(conn, &kit.id)?;
  Ok(kit)
}

fn validate_name(name: &str) -> Result<()> {
  if name.trim().is_empty() {
    return Err(Error::InvalidValue("kit name is empty".to_string()));
  }
  Ok(())
}

pub fn create(conn: &Connection, name: &str, spec: &str) -> Result<Kit> {
  validate_name(name)?;
  let id = new_id();
  let now = now_millis();
  conn.execute(
    "INSERT INTO kits (id, name, spec, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
    params![id, name.trim(), spec, now],
  )?;
  get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> Result<Kit> {
  let kit = conn
    .query_row("SELECT * FROM kits WHERE id = ?1", params![id], row_to_kit)
    .map_err(|e| match e {
      rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
      other => Error::Sqlite(other),
    })?;
  with_projects(conn, kit)
}

pub fn list(conn: &Connection) -> Result<Vec<Kit>> {
  let mut stmt = conn.prepare("SELECT * FROM kits ORDER BY created_at")?;
  let kits = stmt.query_map([], row_to_kit)?.collect::<rusqlite::Result<Vec<_>>>()?;
  kits.into_iter().map(|k| with_projects(conn, k)).collect()
}

pub fn update(conn: &Connection, id: &str, name: &str, spec: &str) -> Result<Kit> {
  validate_name(name)?;
  let changed = conn.execute(
    "UPDATE kits SET name = ?1, spec = ?2, updated_at = ?3 WHERE id = ?4",
    params![name.trim(), spec, now_millis(), id],
  )?;
  if changed == 0 {
    return Err(Error::NotFound);
  }
  get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
  conn.execute("DELETE FROM kits WHERE id = ?1", params![id])?;
  Ok(())
}

/// Global and project scope are exclusive. Assigning a project here takes it from any other kit.
pub fn set_scope(conn: &Connection, id: &str, global: bool, project_ids: &[String]) -> Result<Kit> {
  let tx = conn.unchecked_transaction()?;
  get(&tx, id)?;
  tx.execute("UPDATE projects SET kit_id = NULL WHERE kit_id = ?1", params![id])?;
  if global {
    tx.execute("UPDATE kits SET is_global = (id = ?1)", params![id])?;
  } else {
    tx.execute("UPDATE kits SET is_global = 0 WHERE id = ?1", params![id])?;
    for project_id in project_ids {
      if tx.execute("UPDATE projects SET kit_id = ?1 WHERE id = ?2", params![id, project_id])? == 0 {
        return Err(Error::NotFound);
      }
    }
  }
  tx.execute("UPDATE kits SET updated_at = ?1 WHERE id = ?2", params![now_millis(), id])?;
  tx.commit()?;
  get(conn, id)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;
  use crate::db::projects;

  #[test]
  fn create_then_list() {
    let conn = test_conn();
    let kit = create(&conn, "node", "kind: mixin").unwrap();
    let all = list(&conn).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, kit.id);
    assert!(!all[0].is_global);
    assert!(all[0].project_ids.is_empty());
  }

  #[test]
  fn empty_name_rejected() {
    let conn = test_conn();
    assert!(create(&conn, "  ", "x").is_err());
  }

  #[test]
  fn global_is_exclusive() {
    let conn = test_conn();
    let a = create(&conn, "a", "").unwrap();
    let b = create(&conn, "b", "").unwrap();
    set_scope(&conn, &a.id, true, &[]).unwrap();
    set_scope(&conn, &b.id, true, &[]).unwrap();
    assert!(!get(&conn, &a.id).unwrap().is_global);
    assert!(get(&conn, &b.id).unwrap().is_global);
  }

  #[test]
  fn project_moves_between_kits() {
    let conn = test_conn();
    let p = projects::create(&conn, "p", "/tmp/p", None, None).unwrap();
    let a = create(&conn, "a", "").unwrap();
    let b = create(&conn, "b", "").unwrap();
    set_scope(&conn, &a.id, false, std::slice::from_ref(&p.id)).unwrap();
    set_scope(&conn, &b.id, false, std::slice::from_ref(&p.id)).unwrap();
    assert!(get(&conn, &a.id).unwrap().project_ids.is_empty());
    assert_eq!(get(&conn, &b.id).unwrap().project_ids, vec![p.id.clone()]);
    assert_eq!(projects::get(&conn, &p.id).unwrap().kit_id, Some(b.id));
  }

  #[test]
  fn global_clears_projects() {
    let conn = test_conn();
    let p = projects::create(&conn, "p", "/tmp/p", None, None).unwrap();
    let a = create(&conn, "a", "").unwrap();
    set_scope(&conn, &a.id, false, std::slice::from_ref(&p.id)).unwrap();
    let a = set_scope(&conn, &a.id, true, &[]).unwrap();
    assert!(a.is_global);
    assert!(a.project_ids.is_empty());
  }

  #[test]
  fn delete_unassigns_project() {
    let conn = test_conn();
    let p = projects::create(&conn, "p", "/tmp/p", None, None).unwrap();
    let a = create(&conn, "a", "").unwrap();
    set_scope(&conn, &a.id, false, std::slice::from_ref(&p.id)).unwrap();
    delete(&conn, &a.id).unwrap();
    assert_eq!(projects::get(&conn, &p.id).unwrap().kit_id, None);
  }
}
