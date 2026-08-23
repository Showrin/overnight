use rusqlite::{params, Connection};

use crate::db::error::Result;
use crate::db::models::{new_id, now_millis, HostMetric};
use crate::sbx::HostStats;

fn row_to_host_metric(row: &rusqlite::Row) -> rusqlite::Result<HostMetric> {
  Ok(HostMetric {
    id: row.get("id")?,
    captured_at: row.get("captured_at")?,
    cpu_percent: row.get("cpu_percent")?,
    memory_percent: row.get("memory_percent")?,
    memory_used_mb: row.get("memory_used_mb")?,
    memory_total_mb: row.get("memory_total_mb")?,
    disk_percent: row.get("disk_percent")?,
    disk_used_mb: row.get("disk_used_mb")?,
    disk_total_mb: row.get("disk_total_mb")?,
    network_rx_kb_per_sec: row.get("network_rx_kb_per_sec")?,
    network_tx_kb_per_sec: row.get("network_tx_kb_per_sec")?,
  })
}

pub fn record(conn: &Connection, stats: &HostStats) -> Result<HostMetric> {
  let id = new_id();
  let now = now_millis();
  conn.execute(
    "INSERT INTO host_metrics (
       id, captured_at, cpu_percent, memory_percent, memory_used_mb, memory_total_mb,
       disk_percent, disk_used_mb, disk_total_mb, network_rx_kb_per_sec, network_tx_kb_per_sec
     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    params![
      id,
      now,
      stats.cpu_percent,
      stats.memory_percent,
      stats.memory_used_mb,
      stats.memory_total_mb,
      stats.disk_percent,
      stats.disk_used_mb,
      stats.disk_total_mb,
      stats.network_rx_kb_per_sec,
      stats.network_tx_kb_per_sec,
    ],
  )?;
  let metric = conn.query_row("SELECT * FROM host_metrics WHERE id = ?1", params![id], row_to_host_metric)?;
  Ok(metric)
}

pub fn list_since(conn: &Connection, since_millis: i64) -> Result<Vec<HostMetric>> {
  let mut stmt =
    conn.prepare("SELECT * FROM host_metrics WHERE captured_at >= ?1 ORDER BY captured_at ASC")?;
  let rows = stmt.query_map(params![since_millis], row_to_host_metric)?;
  Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn prune_older_than(conn: &Connection, cutoff_millis: i64) -> Result<()> {
  conn.execute("DELETE FROM host_metrics WHERE captured_at < ?1", params![cutoff_millis])?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::db::migrations::test_conn;

  fn stats(cpu_percent: f64) -> HostStats {
    HostStats {
      cpu_percent,
      memory_percent: 40.0,
      memory_used_mb: 8000.0,
      memory_total_mb: 16000.0,
      disk_percent: 50.0,
      disk_used_mb: 256_000.0,
      disk_total_mb: 512_000.0,
      network_rx_kb_per_sec: 12.0,
      network_tx_kb_per_sec: 3.0,
    }
  }

  #[test]
  fn record_and_list_since() {
    let conn = test_conn();
    record(&conn, &stats(12.5)).unwrap();
    record(&conn, &stats(20.0)).unwrap();

    let history = list_since(&conn, 0).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].cpu_percent, 12.5);
    assert_eq!(history[1].cpu_percent, 20.0);
    assert_eq!(history[1].disk_percent, 50.0);
    assert_eq!(history[1].network_rx_kb_per_sec, 12.0);
  }

  #[test]
  fn prune_removes_only_old_rows() {
    let conn = test_conn();
    conn
      .execute(
        "INSERT INTO host_metrics (id, captured_at, cpu_percent, memory_percent, memory_used_mb, memory_total_mb)
         VALUES ('old', 1000, 1.0, 1.0, 1.0, 1.0)",
        [],
      )
      .unwrap();
    conn
      .execute(
        "INSERT INTO host_metrics (id, captured_at, cpu_percent, memory_percent, memory_used_mb, memory_total_mb)
         VALUES ('new', 5000, 2.0, 2.0, 2.0, 2.0)",
        [],
      )
      .unwrap();

    prune_older_than(&conn, 3000).unwrap();

    let remaining = list_since(&conn, 0).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "new");
  }
}
