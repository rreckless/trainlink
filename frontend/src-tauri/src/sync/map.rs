//! Converting rows between the local database and the cloud. Local integer ids become the cloud's uuids, and
//! references (a block's layout, a route's blocks) are translated both ways. Applying a cloud row follows the rule:
//! a local change that is still waiting to sync wins only if it is newer than the cloud's version

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use super::time::{normalize, to_cloud};
use crate::db::sync as bookkeeping;

pub const PICTURE_EXTENSIONS: [&str; 3] = ["jpg", "png", "webp"];

// Ids -------------------------------------------------------------------

pub fn sync_id_of(conn: &Connection, table: &str, id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row(&format!("SELECT sync_id FROM {table} WHERE id = ?1"), [id], |row| row.get::<_, Option<String>>(0))
        .optional()
        .map(Option::flatten)
}

pub fn local_id_of(conn: &Connection, table: &str, sync_id: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(&format!("SELECT id FROM {table} WHERE sync_id = ?1"), [sync_id], |row| row.get(0)).optional()
}

/// Every row gets a cloud id before anything is sent, so references can be written
pub fn ensure_sync_ids(conn: &Connection, table: &str) -> rusqlite::Result<()> {
    let ids: Vec<i64> = conn.prepare(&format!("SELECT id FROM {table} WHERE sync_id IS NULL"))?.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
    for id in ids {
        conn.execute(&format!("UPDATE {table} SET sync_id = ?1, dirty = 1 WHERE id = ?2"), params![bookkeeping::new_sync_id(), id])?;
    }
    Ok(())
}

// Decisions ---------------------------------------------------------------

/// Apply the cloud's version? None: the local change is newer and still waiting, so keep it. Some(local id):
/// update that row. Some(None): the row is new here
pub fn decide(conn: &Connection, table: &str, sync_id: &str, remote_updated: &str) -> rusqlite::Result<Option<Option<i64>>> {
    let local: Option<(i64, String, bool)> = conn
        .query_row(
            &format!("SELECT id, updated_at, dirty FROM {table} WHERE sync_id = ?1"),
            [sync_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? == 1)),
        )
        .optional()?;
    Ok(match local {
        None => Some(None),
        Some((_, local_updated, true)) if normalize(&local_updated) > normalize(remote_updated) => None,
        Some((id, _, _)) => Some(Some(id)),
    })
}

fn text(row: &Value, key: &str) -> Option<String> {
    row.get(key).and_then(Value::as_str).map(str::to_string)
}

fn float(row: &Value, key: &str) -> f64 {
    row.get(key).and_then(Value::as_f64).unwrap_or_default()
}

fn int(row: &Value, key: &str) -> i64 {
    row.get(key).and_then(Value::as_i64).unwrap_or_default()
}

fn json_text(row: &Value, key: &str) -> Option<String> {
    match row.get(key) {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.to_string()),
    }
}

// Layouts ------------------------------------------------------------------

pub fn apply_layout(conn: &Connection, row: &Value) -> rusqlite::Result<bool> {
    let sync_id = text(row, "id").unwrap_or_default();
    let Some(decision) = decide(conn, "layouts", &sync_id, &text(row, "updated_at").unwrap_or_default())? else { return Ok(false) };
    let elements = json_text(row, "elements").unwrap_or_else(|| r#"{"tracks": []}"#.into());
    let created = normalize(&text(row, "created_at").unwrap_or_default());
    let updated = normalize(&text(row, "updated_at").unwrap_or_default());
    match decision {
        None => conn.execute(
            "INSERT INTO layouts (sync_id, name, description, width, height, scale, track_gauge, elements, created_at, updated_at, dirty)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)",
            params![sync_id, text(row, "name"), text(row, "description"), float(row, "width"), float(row, "height"), text(row, "scale"), text(row, "track_gauge"), elements, created, updated],
        )?,
        Some(id) => conn.execute(
            "UPDATE layouts SET name = ?2, description = ?3, width = ?4, height = ?5, scale = ?6, track_gauge = ?7, elements = ?8, updated_at = ?9, dirty = 0 WHERE id = ?1",
            params![id, text(row, "name"), text(row, "description"), float(row, "width"), float(row, "height"), text(row, "scale"), text(row, "track_gauge"), elements, updated],
        )?,
    };
    Ok(true)
}

pub fn layout_outbound(conn: &Connection, user: &str) -> rusqlite::Result<Vec<(i64, String, Value)>> {
    let mut statement = conn.prepare("SELECT id, sync_id, name, description, width, height, scale, track_gauge, elements, created_at, updated_at FROM layouts WHERE dirty = 1")?;
    let rows = statement.query_map([], |row| {
        let elements: String = row.get(8)?;
        let stamp: String = row.get(10)?;
        Ok((
            row.get::<_, i64>(0)?,
            stamp.clone(),
            json!({
                "id": row.get::<_, String>(1)?,
                "user_id": user,
                "name": row.get::<_, String>(2)?,
                "description": row.get::<_, Option<String>>(3)?,
                "width": row.get::<_, f64>(4)?,
                "height": row.get::<_, f64>(5)?,
                "scale": row.get::<_, String>(6)?,
                "track_gauge": row.get::<_, Option<String>>(7)?,
                "elements": serde_json::from_str::<Value>(&elements).unwrap_or(Value::Null),
                "created_at": to_cloud(&row.get::<_, String>(9)?),
                "updated_at": to_cloud(&stamp),
                "deleted_at": Value::Null,
            }),
        ))
    })?;
    rows.collect()
}

// Locomotives --------------------------------------------------------------------

/// The cloud's picture path for a local locomotive picture
pub fn picture_path(user: &str, sync_id: &str, file: &str) -> String {
    let extension = file.rsplit('.').next().unwrap_or("jpg");
    format!("{user}/{sync_id}.{extension}")
}

pub fn apply_locomotive(conn: &Connection, row: &Value, images: &Path, downloads: &mut Vec<(i64, String)>) -> rusqlite::Result<bool> {
    let sync_id = text(row, "id").unwrap_or_default();
    let Some(decision) = decide(conn, "locomotives", &sync_id, &text(row, "updated_at").unwrap_or_default())? else { return Ok(false) };
    let labels = json_text(row, "function_labels");
    let created = normalize(&text(row, "created_at").unwrap_or_default());
    let updated = normalize(&text(row, "updated_at").unwrap_or_default());
    let id = match decision {
        None => {
            conn.execute(
                "INSERT INTO locomotives (sync_id, name, dcc_address, locomotive_type, length, function_labels, created_at, updated_at, dirty)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
                params![sync_id, text(row, "name"), int(row, "dcc_address"), text(row, "locomotive_type"), row.get("length").and_then(Value::as_f64), labels, created, updated],
            )?;
            conn.last_insert_rowid()
        }
        Some(id) => {
            conn.execute(
                "UPDATE locomotives SET name = ?2, dcc_address = ?3, locomotive_type = ?4, length = ?5, function_labels = ?6, updated_at = ?7, dirty = 0 WHERE id = ?1",
                params![id, text(row, "name"), int(row, "dcc_address"), text(row, "locomotive_type"), row.get("length").and_then(Value::as_f64), labels, updated],
            )?;
            id
        }
    };
    match text(row, "picture_path") {
        None => {
            // The picture was removed elsewhere
            if let Some(file) = conn.query_row("SELECT image_file FROM locomotives WHERE id = ?1", [id], |row| row.get::<_, Option<String>>(0))? {
                let _ = std::fs::remove_file(images.join(&file));
                conn.execute("UPDATE locomotives SET image_file = NULL WHERE id = ?1", [id])?;
            }
        }
        Some(path) => {
            let have = conn.query_row("SELECT image_file FROM locomotives WHERE id = ?1", [id], |row| row.get::<_, Option<String>>(0))?;
            let present = have.as_ref().is_some_and(|file| images.join(file).is_file());
            if !present {
                downloads.push((id, path));
            }
        }
    }
    Ok(true)
}

pub fn locomotive_outbound(conn: &Connection, user: &str, images: &Path, uploads: &mut Vec<(String, std::path::PathBuf)>) -> rusqlite::Result<Vec<(i64, String, Value)>> {
    let mut statement = conn.prepare("SELECT id, sync_id, name, dcc_address, locomotive_type, length, function_labels, image_file, created_at, updated_at FROM locomotives WHERE dirty = 1")?;
    let rows: Vec<_> = statement
        .query_map([], |row| {
            let labels: Option<String> = row.get(6)?;
            let stamp: String = row.get(9)?;
            Ok((
                row.get::<_, i64>(0)?,
                stamp.clone(),
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<f64>>(5)?,
                labels,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for (id, stamp, sync_id, name, address, kind, length, labels, image, created) in rows {
        let picture = image.as_ref().map(|file| picture_path(user, &sync_id, file));
        if let (Some(file), Some(path)) = (&image, &picture) {
            uploads.push((path.clone(), images.join(file)));
        }
        out.push((
            id,
            stamp.clone(),
            json!({
                "id": sync_id,
                "user_id": user,
                "name": name,
                "dcc_address": address,
                "locomotive_type": kind,
                "length": length,
                "function_labels": labels.and_then(|text| serde_json::from_str::<Value>(&text).ok()),
                "picture_path": picture,
                "created_at": to_cloud(&created),
                "updated_at": to_cloud(&stamp),
                "deleted_at": Value::Null,
            }),
        ));
    }
    Ok(out)
}
