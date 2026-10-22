//! Settings in the sync. Each setting is one row, and the newer change wins, as for the other tables

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::map::decide;
use super::time::{normalize, to_cloud};

fn text(row: &Value, key: &str) -> String {
    row.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// Applies a setting from the cloud, unless this device's change is newer
pub fn apply_setting(conn: &Connection, row: &Value) -> rusqlite::Result<bool> {
    let sync_id = text(row, "id");
    let remote_updated = text(row, "updated_at");
    let Some(decision) = decide(conn, "settings", &sync_id, &remote_updated)? else { return Ok(false) };
    let name = text(row, "name");
    let value = row.get("value").cloned().unwrap_or(Value::Null).to_string();
    let updated = normalize(&remote_updated);
    match decision {
        None => conn.execute(
            "INSERT INTO settings (sync_id, name, value, created_at, updated_at, dirty) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            params![sync_id, name, value, normalize(&text(row, "created_at")), updated],
        )?,
        Some(id) => conn.execute(
            "UPDATE settings SET name = ?2, value = ?3, updated_at = ?4, dirty = 0 WHERE id = ?1",
            params![id, name, value, updated],
        )?,
    };
    Ok(true)
}

/// The settings this device changed, ready to send
pub fn setting_outbound(conn: &Connection, user: &str) -> rusqlite::Result<Vec<(i64, String, Value)>> {
    let mut statement = conn.prepare("SELECT id, sync_id, name, value, created_at, updated_at FROM settings WHERE dirty = 1")?;
    let rows = statement.query_map([], |row| {
        let value: String = row.get(3)?;
        let stamp: String = row.get(5)?;
        Ok((
            row.get::<_, i64>(0)?,
            stamp.clone(),
            json!({
                "id": row.get::<_, String>(1)?,
                "user_id": user,
                "name": row.get::<_, String>(2)?,
                "value": serde_json::from_str::<Value>(&value).unwrap_or(Value::Null),
                "created_at": to_cloud(&row.get::<_, String>(4)?),
                "updated_at": to_cloud(&stamp),
                "deleted_at": Value::Null,
            }),
        ))
    })?;
    rows.collect()
}
