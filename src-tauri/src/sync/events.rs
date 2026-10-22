//! Moving time as events. Each event adds seconds to a locomotive once, on the device that recorded it and on
//! every other device that receives it

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::map::{local_id_of, sync_id_of};
use super::time::{normalize, to_cloud};

/// Events recorded here that have not been sent yet
pub fn outbound(conn: &Connection, user: &str) -> rusqlite::Result<Vec<(i64, String, Value)>> {
    let mut statement = conn.prepare("SELECT id, sync_id, locomotive_id, device_id, seconds, recorded_at FROM moving_time_events WHERE dirty = 1")?;
    let rows: Vec<_> = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?, row.get::<_, f64>(4)?, row.get::<_, String>(5)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for (id, sync_id, locomotive, device, seconds, recorded) in rows {
        // An event for a locomotive that has no cloud id yet waits until it has one
        let Some(locomotive_sync) = sync_id_of(conn, "locomotives", locomotive)? else { continue };
        out.push((id, recorded.clone(), json!({
            "id": sync_id, "user_id": user, "locomotive_id": locomotive_sync, "device_id": device,
            "seconds": seconds, "recorded_at": to_cloud(&recorded),
        })));
    }
    Ok(out)
}

/// Add an event from the cloud, once. Returns true if it was new here
pub fn apply(conn: &Connection, row: &Value) -> rusqlite::Result<bool> {
    let sync_id = row.get("id").and_then(Value::as_str).unwrap_or_default();
    let known: Option<i64> = conn
        .query_row("SELECT id FROM moving_time_events WHERE sync_id = ?1", [sync_id], |row| row.get(0))
        .ok();
    if known.is_some() {
        return Ok(false);
    }
    let Some(locomotive_sync) = row.get("locomotive_id").and_then(Value::as_str) else { return Ok(false) };
    let Some(locomotive) = local_id_of(conn, "locomotives", locomotive_sync)? else { return Ok(false) };
    let seconds = row.get("seconds").and_then(Value::as_f64).unwrap_or_default();
    conn.execute(
        "INSERT INTO moving_time_events (sync_id, locomotive_id, device_id, seconds, recorded_at, dirty) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        params![
            sync_id,
            locomotive,
            row.get("device_id").and_then(Value::as_str).unwrap_or_default(),
            seconds,
            normalize(row.get("recorded_at").and_then(Value::as_str).unwrap_or_default()),
        ],
    )?;
    conn.execute("UPDATE locomotives SET travelled_time = travelled_time + ?2 WHERE id = ?1", params![locomotive, seconds])?;
    Ok(true)
}
