//! Locomotive rows, including the picture file name and the function button labels

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};

use super::{now, sync};

/// The 20 function buttons, with F0 labelled as the light
pub const FUNCTION_BUTTONS: usize = 20;

#[derive(Debug, Clone, Serialize)]
pub struct Locomotive {
    pub id: i64,
    pub name: String,
    pub dcc_address: i64,
    pub locomotive_type: String,
    pub length: Option<f64>,
    pub speed: i64,
    pub direction: String,
    pub functions: i64,
    pub function_labels: Value,
    pub image_url: Option<String>,
    pub travelled_time: f64,
    pub travelled_distance: f64,
    pub created_at: String,
    #[serde(skip)]
    pub image_file: Option<String>,
    #[serde(skip)]
    pub command_station_id: Option<i64>,
    /// The decoder profile this locomotive's decoder follows, if one is chosen
    pub decoder_profile_id: Option<i64>,
}

/// Labels for a locomotive that has none: Light, then F1 to F19
pub fn default_labels() -> Value {
    let mut labels = vec![json!({ "label": "Light", "momentary": false })];
    for index in 1..FUNCTION_BUTTONS {
        labels.push(json!({ "label": format!("F{index}"), "momentary": false }));
    }
    Value::Array(labels)
}

const COLUMNS: &str = "id, name, dcc_address, locomotive_type, length, speed, direction, functions,
    function_labels, image_file, travelled_time, travelled_distance, command_station_id, decoder_profile_id, created_at";

fn from_row(row: &Row) -> rusqlite::Result<Locomotive> {
    let id: i64 = row.get("id")?;
    let image_file: Option<String> = row.get("image_file")?;
    let stored: Option<String> = row.get("function_labels")?;
    let labels = stored
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(|value| value.as_array().is_some_and(|items| !items.is_empty()))
        .unwrap_or_else(default_labels);
    Ok(Locomotive {
        id,
        name: row.get("name")?,
        dcc_address: row.get("dcc_address")?,
        locomotive_type: row.get("locomotive_type")?,
        length: row.get("length")?,
        speed: row.get("speed")?,
        direction: row.get("direction")?,
        functions: row.get("functions")?,
        function_labels: labels,
        image_url: image_file.as_ref().map(|_| format!("/api/locomotives/{id}/image")),
        travelled_time: row.get("travelled_time")?,
        travelled_distance: row.get("travelled_distance")?,
        created_at: row.get("created_at")?,
        image_file,
        command_station_id: row.get("command_station_id")?,
        decoder_profile_id: row.get("decoder_profile_id")?,
    })
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Locomotive>> {
    let mut statement = conn.prepare(&format!("SELECT {COLUMNS} FROM locomotives ORDER BY id"))?;
    let rows = statement.query_map([], from_row)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Locomotive>> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM locomotives WHERE id = ?1"), [id], from_row)
        .optional()
}

/// The locomotive with this DCC address, if one exists
pub fn by_address(conn: &Connection, address: i64) -> rusqlite::Result<Option<Locomotive>> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM locomotives WHERE dcc_address = ?1"),
        [address],
        from_row,
    )
    .optional()
}

pub struct NewLocomotive<'a> {
    pub name: &'a str,
    pub dcc_address: i64,
    pub locomotive_type: &'a str,
    pub length: Option<f64>,
    pub command_station_id: Option<i64>,
}

pub fn insert(conn: &Connection, locomotive: &NewLocomotive) -> rusqlite::Result<i64> {
    let stamp = now();
    conn.execute(
        "INSERT INTO locomotives (name, dcc_address, locomotive_type, length, command_station_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            locomotive.name,
            locomotive.dcc_address,
            locomotive.locomotive_type,
            locomotive.length,
            locomotive.command_station_id,
            stamp
        ],
    )?;
    let id = conn.last_insert_rowid();
    sync::mark_new(conn, "locomotives", id)?;
    Ok(id)
}

/// Write the editable columns back. Function labels are stored only when the locomotive has its own
pub fn save(conn: &Connection, locomotive: &Locomotive) -> rusqlite::Result<()> {
    let labels = if locomotive.function_labels == default_labels() {
        None
    } else {
        Some(locomotive.function_labels.to_string())
    };
    conn.execute(
        "UPDATE locomotives SET name = ?2, dcc_address = ?3, locomotive_type = ?4, length = ?5,
             functions = ?6, function_labels = ?7, image_file = ?8, travelled_time = ?9,
             travelled_distance = ?10, decoder_profile_id = ?12, updated_at = ?11 WHERE id = ?1",
        params![
            locomotive.id,
            locomotive.name,
            locomotive.dcc_address,
            locomotive.locomotive_type,
            locomotive.length,
            locomotive.functions,
            labels,
            locomotive.image_file,
            locomotive.travelled_time,
            locomotive.travelled_distance,
            now(),
            locomotive.decoder_profile_id
        ],
    )?;
    sync::mark_changed(conn, "locomotives", locomotive.id)?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    sync::forget(conn, "locomotives", id)?;
    conn.execute("DELETE FROM locomotives WHERE id = ?1", [id])?;
    Ok(())
}

/// True if a block is occupied by this locomotive or a route is running it
pub fn in_use(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM blocks WHERE occupancy_locomotive_id = ?1)
              + (SELECT COUNT(*) FROM routes WHERE current_locomotive_id = ?1)",
        [id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Record seconds of movement for the locomotive with this address. Each run is an event, so every device adds
/// its own time, and the events sync to the others. Returns false if no locomotive has the address
pub fn add_travelled_time(conn: &Connection, address: i64, seconds: f64) -> rusqlite::Result<bool> {
    let Some(id) = conn
        .query_row("SELECT id FROM locomotives WHERE dcc_address = ?1", [address], |row| row.get::<_, i64>(0))
        .optional()?
    else {
        return Ok(false);
    };
    let device = sync::device_id(conn)?;
    conn.execute(
        "INSERT INTO moving_time_events (sync_id, locomotive_id, device_id, seconds, recorded_at, dirty) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        params![sync::new_sync_id(), id, device, seconds, now()],
    )?;
    conn.execute(
        "UPDATE locomotives SET travelled_time = travelled_time + ?2, updated_at = ?3 WHERE id = ?1",
        params![id, seconds, now()],
    )?;
    Ok(true)
}
