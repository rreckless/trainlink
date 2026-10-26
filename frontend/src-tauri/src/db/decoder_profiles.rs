//! Decoder profiles: the CVs a decoder has, with each one's name, range and whether it can be written, and the names of its
//! functions. A locomotive can use one. The NMRA standard profile is built in and cannot be deleted

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};

use super::now;

/// The CVs every profile starts from: the NMRA standard numbers and ranges, with the names used in the NMRA
/// standard. Read-only CVs are marked, and cannot be written
pub const NMRA_CVS: &str = r#"[
  {"cv":1,"name":"Short address","min":1,"max":127,"read_only":false},
  {"cv":2,"name":"Start voltage (minimum speed)","min":0,"max":255,"read_only":false},
  {"cv":3,"name":"Acceleration","min":0,"max":255,"read_only":false},
  {"cv":4,"name":"Deceleration","min":0,"max":255,"read_only":false},
  {"cv":5,"name":"Maximum speed","min":0,"max":255,"read_only":false},
  {"cv":6,"name":"Middle speed","min":0,"max":255,"read_only":false},
  {"cv":7,"name":"Version","min":0,"max":255,"read_only":true},
  {"cv":8,"name":"Manufacturer","min":0,"max":255,"read_only":true},
  {"cv":17,"name":"Long address, high byte","min":192,"max":231,"read_only":false},
  {"cv":18,"name":"Long address, low byte","min":0,"max":255,"read_only":false},
  {"cv":19,"name":"Consist address (multiple unit)","min":0,"max":127,"read_only":false},
  {"cv":28,"name":"RailCom configuration","min":0,"max":255,"read_only":false},
  {"cv":29,"name":"Configuration (bits)","min":0,"max":255,"read_only":false}
]"#;

pub const NMRA_NAME: &str = "NMRA standard";

#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub manufacturer: String,
    pub description: String,
    pub builtin: bool,
    pub cvs: Value,
    pub functions: Value,
}

const COLUMNS: &str = "id, name, manufacturer, description, builtin, cvs, functions";

fn from_row(row: &Row) -> rusqlite::Result<Profile> {
    let cvs: String = row.get("cvs")?;
    let functions: String = row.get("functions")?;
    Ok(Profile {
        id: row.get("id")?,
        name: row.get("name")?,
        manufacturer: row.get("manufacturer")?,
        description: row.get("description")?,
        builtin: row.get::<_, i64>("builtin")? != 0,
        cvs: serde_json::from_str(&cvs).unwrap_or_else(|_| json!([])),
        functions: serde_json::from_str(&functions).unwrap_or_else(|_| json!([])),
    })
}

/// Add the NMRA standard profile, if it is not there yet
pub fn seed(conn: &Connection) -> rusqlite::Result<()> {
    let stamp = now();
    conn.execute(
        "INSERT OR IGNORE INTO decoder_profiles (name, manufacturer, description, builtin, cvs, functions, created_at, updated_at)
         VALUES (?1, 'NMRA', 'The CVs of the NMRA standard, which most decoders follow', 1, ?2, '[]', ?3, ?3)",
        params![NMRA_NAME, NMRA_CVS, stamp],
    )?;
    Ok(())
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Profile>> {
    let mut statement = conn.prepare(&format!("SELECT {COLUMNS} FROM decoder_profiles ORDER BY builtin DESC, name"))?;
    let rows = statement.query_map([], from_row)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Profile>> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM decoder_profiles WHERE id = ?1"), [id], from_row).optional()
}

pub fn insert(conn: &Connection, name: &str, manufacturer: &str, description: &str, cvs: &Value, functions: &Value) -> rusqlite::Result<i64> {
    let stamp = now();
    conn.execute(
        "INSERT INTO decoder_profiles (name, manufacturer, description, builtin, cvs, functions, created_at, updated_at)
         VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?6)",
        params![name, manufacturer, description, cvs.to_string(), functions.to_string(), stamp],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Delete a profile. Locomotives that used it go back to no profile. The built-in profile is not deleted
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE locomotives SET decoder_profile_id = NULL WHERE decoder_profile_id = ?1", [id])?;
    conn.execute("DELETE FROM decoder_profiles WHERE id = ?1 AND builtin = 0", [id])?;
    Ok(())
}

/// The rule for one CV in a profile: its range and whether it can be written. None if the profile does not have that CV
pub fn cv_rule(profile: &Value, cv: u16) -> Option<&Value> {
    profile.as_array()?.iter().find(|rule| rule.get("cv").and_then(Value::as_u64) == Some(u64::from(cv)))
}
