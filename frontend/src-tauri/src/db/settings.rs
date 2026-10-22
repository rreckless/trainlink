//! Settings for the account: the Z21 address, and any setting added later. Each setting is one row. Its cloud id comes
//! from its name, so two devices that set the same setting meet on the same row

use rusqlite::{params, Connection};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::now;

/// The settings the app knows. Any other name is refused
pub const NAMES: [&str; 2] = ["z21.host", "z21.port"];

/// Namespace for the cloud ids of settings: the same name gives the same id on every device
const NAMESPACE: Uuid = Uuid::from_u128(0x3f1c_2b7e_8d4a_4e6b_9a51_7c2d_0e9f_4b10);

/// The cloud id of a setting, from its name
pub fn sync_id_of(name: &str) -> String {
    Uuid::new_v5(&NAMESPACE, name.as_bytes()).to_string()
}

/// Every setting that has a value, by name
pub fn all(conn: &Connection) -> rusqlite::Result<Map<String, Value>> {
    let mut statement = conn.prepare("SELECT name, value FROM settings ORDER BY name")?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    let mut map = Map::new();
    for row in rows {
        let (name, value) = row?;
        map.insert(name, serde_json::from_str(&value).unwrap_or(Value::Null));
    }
    Ok(map)
}

/// Sets a setting's value, and marks it changed so that it is sent to the cloud
pub fn set(conn: &Connection, name: &str, value: &Value) -> rusqlite::Result<()> {
    let stamp = now();
    conn.execute(
        "INSERT INTO settings (name, value, sync_id, created_at, updated_at, dirty) VALUES (?1, ?2, ?3, ?4, ?4, 1)
         ON CONFLICT(name) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at, dirty = 1",
        params![name, value.to_string(), sync_id_of(name), stamp],
    )?;
    Ok(())
}
