//! Locomotive rows, including the picture file name and the function button labels

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};

use super::{now, sync};

/// The 20 function buttons, with F0 labelled as the light
pub const FUNCTION_BUTTONS: usize = 20;

/// The kinds of locomotive a roster entry can be
pub const LOCOMOTIVE_TYPES: [&str; 8] = ["steam", "diesel", "electric", "multiple unit", "tram", "shunt", "museum", "other"];

/// The highest speed step DCC uses
pub const MAX_SPEED_STEP: i64 = 126;

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
    pub command_station_id: Option<i64>,
    /// The decoder profile this locomotive's decoder follows, if one is chosen
    pub decoder_profile_id: Option<i64>,
    /// The group the locomotive belongs to on the roster, if any
    pub group_name: Option<String>,
    /// The highest speed step the locomotive may be driven at, if it is limited
    pub max_speed: Option<i64>,
    /// True when forward on the roster is sent to the decoder as reverse, for a locomotive fitted the other way round
    pub reverse_direction: bool,
    /// The colour shown for the locomotive, as #rrggbb
    pub colour: Option<String>,
}

/// Labels for a locomotive that has none: Light, then F1 to F19
pub fn default_labels() -> Value {
    let mut labels = vec![json!({ "label": "Light", "momentary": false })];
    for index in 1..FUNCTION_BUTTONS {
        labels.push(json!({ "label": format!("F{index}"), "momentary": false }));
    }
    Value::Array(labels)
}

/// The default labels, with the names a decoder profile gives its functions. Functions past the buttons are left out
pub fn labels_from_profile(functions: &Value) -> Value {
    let mut labels = default_labels();
    for function in functions.as_array().into_iter().flatten() {
        let index = function.get("index").and_then(Value::as_u64).unwrap_or(u64::MAX);
        let Some(label) = function.get("label").and_then(Value::as_str) else { continue };
        if let Some(slot) = labels.get_mut(index as usize).filter(|_| index < FUNCTION_BUTTONS as u64) {
            slot["label"] = json!(label);
        }
    }
    labels
}

/// The lowest DCC address from 1 that no locomotive on this command station uses. A locomotive with no station shares
/// one set of addresses with the other locomotives that have none
pub fn next_free_address(conn: &Connection, command_station_id: Option<i64>) -> rusqlite::Result<i64> {
    let mut statement = conn.prepare(
        "SELECT dcc_address FROM locomotives WHERE COALESCE(command_station_id, 0) = COALESCE(?1, 0) ORDER BY dcc_address",
    )?;
    let used: Vec<i64> = statement.query_map([command_station_id], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(used.iter().enumerate().find(|(index, address)| **address != *index as i64 + 1).map_or(used.len() as i64 + 1, |(index, _)| index as i64 + 1))
}

/// The name of the locomotive already using this address on this command station, other than the one given
pub fn address_user(conn: &Connection, address: i64, command_station_id: Option<i64>, except: Option<i64>) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT name FROM locomotives WHERE dcc_address = ?1 AND COALESCE(command_station_id, 0) = COALESCE(?2, 0) AND id IS NOT ?3",
        params![address, command_station_id, except],
        |row| row.get(0),
    )
    .optional()
}

const COLUMNS: &str = "id, name, dcc_address, locomotive_type, length, speed, direction, functions,
    function_labels, image_file, travelled_time, travelled_distance, command_station_id, decoder_profile_id, created_at,
    group_name, max_speed, reverse_direction, colour";

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
        group_name: row.get("group_name")?,
        max_speed: row.get("max_speed")?,
        reverse_direction: row.get("reverse_direction")?,
        colour: row.get("colour")?,
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
    pub group_name: Option<&'a str>,
    pub max_speed: Option<i64>,
    pub reverse_direction: bool,
    pub colour: Option<&'a str>,
}

pub fn insert(conn: &Connection, locomotive: &NewLocomotive) -> rusqlite::Result<i64> {
    let stamp = now();
    conn.execute(
        "INSERT INTO locomotives (name, dcc_address, locomotive_type, length, command_station_id, group_name, max_speed,
             reverse_direction, colour, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            locomotive.name,
            locomotive.dcc_address,
            locomotive.locomotive_type,
            locomotive.length,
            locomotive.command_station_id,
            locomotive.group_name,
            locomotive.max_speed,
            locomotive.reverse_direction,
            locomotive.colour,
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
             travelled_distance = ?10, decoder_profile_id = ?12, group_name = ?13, max_speed = ?14,
             reverse_direction = ?15, colour = ?16, updated_at = ?11 WHERE id = ?1",
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
            locomotive.decoder_profile_id,
            locomotive.group_name,
            locomotive.max_speed,
            locomotive.reverse_direction,
            locomotive.colour
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
