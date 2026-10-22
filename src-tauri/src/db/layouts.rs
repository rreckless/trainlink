//! Layout rows. Track is kept in the `elements` JSON, as the web backend did

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};

use super::now;

#[derive(Debug, Clone, Serialize)]
pub struct Layout {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub width: f64,
    pub height: f64,
    pub scale: String,
    pub track_gauge: Option<String>,
    pub elements: Value,
    pub created_at: String,
}

const COLUMNS: &str = "id, name, description, width, height, scale, track_gauge, elements, created_at";

fn from_row(row: &Row) -> rusqlite::Result<Layout> {
    let elements: String = row.get("elements")?;
    Ok(Layout {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        width: row.get("width")?,
        height: row.get("height")?,
        scale: row.get("scale")?,
        track_gauge: row.get("track_gauge")?,
        elements: serde_json::from_str(&elements).unwrap_or_else(|_| json!({ "tracks": [] })),
        created_at: row.get("created_at")?,
    })
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Layout>> {
    let mut statement = conn.prepare(&format!("SELECT {COLUMNS} FROM layouts ORDER BY id"))?;
    let rows = statement.query_map([], from_row)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Layout>> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM layouts WHERE id = ?1"), [id], from_row)
        .optional()
}

pub struct NewLayout<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub width: f64,
    pub height: f64,
    pub scale: &'a str,
    pub track_gauge: Option<&'a str>,
}

pub fn insert(conn: &Connection, layout: &NewLayout) -> rusqlite::Result<i64> {
    let stamp = now();
    conn.execute(
        "INSERT INTO layouts (name, description, width, height, scale, track_gauge, elements, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, '{\"tracks\": []}', ?7, ?7)",
        params![
            layout.name,
            layout.description,
            layout.width,
            layout.height,
            layout.scale,
            layout.track_gauge,
            stamp
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Write every editable column of a layout back to its row
pub fn save(conn: &Connection, layout: &Layout) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE layouts SET name = ?2, description = ?3, width = ?4, height = ?5, scale = ?6,
             track_gauge = ?7, elements = ?8, updated_at = ?9 WHERE id = ?1",
        params![
            layout.id,
            layout.name,
            layout.description,
            layout.width,
            layout.height,
            layout.scale,
            layout.track_gauge,
            layout.elements.to_string(),
            now()
        ],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM layouts WHERE id = ?1", [id])?;
    Ok(())
}

/// The kinds of thing still on a layout, named as the API reports them. Empty if the layout is clear
pub fn contents(conn: &Connection, id: i64) -> rusqlite::Result<Vec<&'static str>> {
    let mut found = Vec::new();
    for (table, label) in [("blocks", "blocks"), ("signals", "signals"), ("feedbacks", "feedbacks"), ("routes", "routes")] {
        let count: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE layout_id = ?1"),
            [id],
            |row| row.get(0),
        )?;
        if count > 0 {
            found.push(label);
        }
    }
    Ok(found)
}
