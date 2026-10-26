//! Command stations: the digital systems TrainLink talks to. Only the Z21 is supported so far (plan Phase 7 adds more).
//! Locomotives and turnouts can refer to one; a station that is in use cannot be deleted

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::Value;

use super::now;

#[derive(Debug, Clone, Serialize)]
pub struct CommandStation {
    pub id: i64,
    pub name: String,
    pub station_type: String,
    pub port: String,
    pub settings: Value,
}

const COLUMNS: &str = "id, name, station_type, port, settings";

fn from_row(row: &Row) -> rusqlite::Result<CommandStation> {
    let settings: String = row.get("settings")?;
    Ok(CommandStation {
        id: row.get("id")?,
        name: row.get("name")?,
        station_type: row.get("station_type")?,
        port: row.get("port")?,
        settings: serde_json::from_str(&settings).unwrap_or_else(|_| Value::Object(Default::default())),
    })
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<CommandStation>> {
    let mut statement = conn.prepare(&format!("SELECT {COLUMNS} FROM command_stations ORDER BY id"))?;
    let rows = statement.query_map([], from_row)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<CommandStation>> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM command_stations WHERE id = ?1"), [id], from_row)
        .optional()
}

pub fn insert(conn: &Connection, name: &str, station_type: &str, port: &str, settings: &Value) -> rusqlite::Result<i64> {
    let stamp = now();
    conn.execute(
        "INSERT INTO command_stations (name, station_type, port, settings, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![name, station_type, port, settings.to_string(), stamp],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Write the editable columns back
pub fn save(conn: &Connection, station: &CommandStation) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE command_stations SET name = ?2, station_type = ?3, port = ?4, settings = ?5, updated_at = ?6 WHERE id = ?1",
        params![
            station.id,
            station.name,
            station.station_type,
            station.port,
            station.settings.to_string(),
            now()
        ],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM command_stations WHERE id = ?1", [id])?;
    Ok(())
}

/// How many locomotives use this station
pub fn locomotives_using(conn: &Connection, id: i64) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM locomotives WHERE command_station_id = ?1", [id], |row| row.get(0))
}
