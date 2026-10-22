//! Blocks, signals, feedbacks, and routes: their rows, and the references between them, in both directions

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::map::{decide, local_id_of, sync_id_of};
use super::time::{normalize, to_cloud};

fn text(row: &Value, key: &str) -> Option<String> {
    row.get(key).and_then(Value::as_str).map(str::to_string)
}

/// The local id of the layout a row belongs to, or None if that layout is not here yet
fn layout_of(conn: &Connection, row: &Value) -> rusqlite::Result<Option<i64>> {
    match text(row, "layout_id") {
        Some(sync_id) => local_id_of(conn, "layouts", &sync_id),
        None => Ok(None),
    }
}

pub fn apply_block(conn: &Connection, row: &Value) -> rusqlite::Result<bool> {
    let sync_id = text(row, "id").unwrap_or_default();
    let Some(layout) = layout_of(conn, row)? else { return Ok(false) };
    let locomotive = match text(row, "occupancy_locomotive_id") {
        Some(sync) => local_id_of(conn, "locomotives", &sync)?,
        None => None,
    };
    let Some(decision) = decide(conn, "blocks", &sync_id, &text(row, "updated_at").unwrap_or_default())? else { return Ok(false) };
    let state = text(row, "state").unwrap_or_else(|| "free".into());
    let occupied = state == "occupied";
    let updated = normalize(&text(row, "updated_at").unwrap_or_default());
    match decision {
        None => {
            conn.execute(
                "INSERT INTO blocks (sync_id, layout_id, name, x, y, max_speed, length, state, occupied, occupancy_locomotive_id, created_at, updated_at, dirty)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0)",
                params![sync_id, layout, text(row, "name"), num(row, "x"), num(row, "y"), int(row, "max_speed"), row.get("length").and_then(Value::as_f64), state, occupied, locomotive, normalize(&text(row, "created_at").unwrap_or_default()), updated],
            )?;
        }
        Some(id) => {
            conn.execute(
                "UPDATE blocks SET layout_id = ?2, name = ?3, x = ?4, y = ?5, max_speed = ?6, length = ?7, state = ?8, occupied = ?9, occupancy_locomotive_id = ?10, updated_at = ?11, dirty = 0 WHERE id = ?1",
                params![id, layout, text(row, "name"), num(row, "x"), num(row, "y"), int(row, "max_speed"), row.get("length").and_then(Value::as_f64), state, occupied, locomotive, updated],
            )?;
        }
    }
    Ok(true)
}

pub fn block_outbound(conn: &Connection, user: &str) -> rusqlite::Result<Vec<(i64, String, Value)>> {
    let mut statement = conn.prepare(
        "SELECT id, sync_id, layout_id, name, x, y, max_speed, length, state, occupancy_locomotive_id, created_at, updated_at FROM blocks WHERE dirty = 1",
    )?;
    let rows: Vec<_> = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<i64>>(9)?,
                row.get::<_, String>(10)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for (id, stamp, sync_id, layout, name, x, y, max_speed, length, state, locomotive, created) in rows {
        let Some(layout_sync) = sync_id_of(conn, "layouts", layout)? else { continue };
        let locomotive_sync = match locomotive {
            Some(local) => sync_id_of(conn, "locomotives", local)?,
            None => None,
        };
        out.push((id, stamp.clone(), json!({
            "id": sync_id,
            "user_id": user,
            "layout_id": layout_sync,
            "name": name,
            "x": x,
            "y": y,
            "max_speed": max_speed,
            "length": length,
            "state": state,
            "occupancy_locomotive_id": locomotive_sync,
            "created_at": to_cloud(&created),
            "updated_at": to_cloud(&stamp),
            "deleted_at": Value::Null,
        })));
    }
    Ok(out)
}

pub fn apply_signal(conn: &Connection, row: &Value) -> rusqlite::Result<bool> {
    let sync_id = text(row, "id").unwrap_or_default();
    let Some(layout) = layout_of(conn, row)? else { return Ok(false) };
    let Some(decision) = decide(conn, "signals", &sync_id, &text(row, "updated_at").unwrap_or_default())? else { return Ok(false) };
    let updated = normalize(&text(row, "updated_at").unwrap_or_default());
    match decision {
        None => conn.execute(
            "INSERT INTO signals (sync_id, layout_id, name, x, y, signal_type, aspect, created_at, updated_at, dirty) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)",
            params![sync_id, layout, text(row, "name"), num(row, "x"), num(row, "y"), text(row, "signal_type"), text(row, "aspect"), normalize(&text(row, "created_at").unwrap_or_default()), updated],
        )?,
        Some(id) => conn.execute(
            "UPDATE signals SET layout_id = ?2, name = ?3, x = ?4, y = ?5, signal_type = ?6, aspect = ?7, updated_at = ?8, dirty = 0 WHERE id = ?1",
            params![id, layout, text(row, "name"), num(row, "x"), num(row, "y"), text(row, "signal_type"), text(row, "aspect"), updated],
        )?,
    };
    Ok(true)
}

pub fn signal_outbound(conn: &Connection, user: &str) -> rusqlite::Result<Vec<(i64, String, Value)>> {
    let mut statement = conn.prepare("SELECT id, sync_id, layout_id, name, x, y, signal_type, aspect, created_at, updated_at FROM signals WHERE dirty = 1")?;
    let rows: Vec<_> = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(9)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?, row.get::<_, f64>(4)?, row.get::<_, f64>(5)?, row.get::<_, String>(6)?, row.get::<_, String>(7)?, row.get::<_, String>(8)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for (id, stamp, sync_id, layout, name, x, y, kind, aspect, created) in rows {
        let Some(layout_sync) = sync_id_of(conn, "layouts", layout)? else { continue };
        out.push((id, stamp.clone(), json!({
            "id": sync_id, "user_id": user, "layout_id": layout_sync, "name": name, "x": x, "y": y,
            "signal_type": kind, "aspect": aspect, "created_at": to_cloud(&created), "updated_at": to_cloud(&stamp), "deleted_at": Value::Null,
        })));
    }
    Ok(out)
}

pub fn apply_feedback(conn: &Connection, row: &Value) -> rusqlite::Result<bool> {
    let sync_id = text(row, "id").unwrap_or_default();
    let Some(layout) = layout_of(conn, row)? else { return Ok(false) };
    let Some(decision) = decide(conn, "feedbacks", &sync_id, &text(row, "updated_at").unwrap_or_default())? else { return Ok(false) };
    let updated = normalize(&text(row, "updated_at").unwrap_or_default());
    let state = row.get("state").and_then(Value::as_bool).unwrap_or(false);
    match decision {
        None => conn.execute(
            "INSERT INTO feedbacks (sync_id, layout_id, name, x, y, feedback_number, state, created_at, updated_at, dirty) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)",
            params![sync_id, layout, text(row, "name"), num(row, "x"), num(row, "y"), int(row, "feedback_number"), state, normalize(&text(row, "created_at").unwrap_or_default()), updated],
        )?,
        Some(id) => conn.execute(
            "UPDATE feedbacks SET layout_id = ?2, name = ?3, x = ?4, y = ?5, feedback_number = ?6, state = ?7, updated_at = ?8, dirty = 0 WHERE id = ?1",
            params![id, layout, text(row, "name"), num(row, "x"), num(row, "y"), int(row, "feedback_number"), state, updated],
        )?,
    };
    Ok(true)
}

pub fn feedback_outbound(conn: &Connection, user: &str) -> rusqlite::Result<Vec<(i64, String, Value)>> {
    let mut statement = conn.prepare("SELECT id, sync_id, layout_id, name, x, y, feedback_number, state, created_at, updated_at FROM feedbacks WHERE dirty = 1")?;
    let rows: Vec<_> = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(9)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?, row.get::<_, f64>(4)?, row.get::<_, f64>(5)?, row.get::<_, i64>(6)?, row.get::<_, bool>(7)?, row.get::<_, String>(8)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for (id, stamp, sync_id, layout, name, x, y, number, state, created) in rows {
        let Some(layout_sync) = sync_id_of(conn, "layouts", layout)? else { continue };
        out.push((id, stamp.clone(), json!({
            "id": sync_id, "user_id": user, "layout_id": layout_sync, "name": name, "x": x, "y": y,
            "feedback_number": number, "state": state, "created_at": to_cloud(&created), "updated_at": to_cloud(&stamp), "deleted_at": Value::Null,
        })));
    }
    Ok(out)
}

pub fn apply_route(conn: &Connection, row: &Value) -> rusqlite::Result<bool> {
    let sync_id = text(row, "id").unwrap_or_default();
    let Some(layout) = layout_of(conn, row)? else { return Ok(false) };
    // Every block in the sequence must be here; otherwise the route waits for a later sync
    let mut sequence = Vec::new();
    for item in row.get("block_sequence").and_then(Value::as_array).cloned().unwrap_or_default() {
        match item.as_str().map(|sync| local_id_of(conn, "blocks", sync)).transpose()?.flatten() {
            Some(local) => sequence.push(local),
            None => return Ok(false),
        }
    }
    let Some(decision) = decide(conn, "routes", &sync_id, &text(row, "updated_at").unwrap_or_default())? else { return Ok(false) };
    let updated = normalize(&text(row, "updated_at").unwrap_or_default());
    let sequence_text = serde_json::to_string(&sequence).unwrap_or_else(|_| "[]".into());
    let speed = row.get("speed_profile").filter(|value| !value.is_null()).map(Value::to_string);
    let dwell = row.get("dwell_times").filter(|value| !value.is_null()).map(Value::to_string);
    match decision {
        None => conn.execute(
            "INSERT INTO routes (sync_id, layout_id, name, block_sequence, speed_profile, dwell_times, auto_return, enabled, is_running, current_block_index, created_at, updated_at, dirty)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0, ?9, ?10, 0)",
            params![sync_id, layout, text(row, "name"), sequence_text, speed, dwell, flag(row, "auto_return"), flag(row, "enabled"), normalize(&text(row, "created_at").unwrap_or_default()), updated],
        )?,
        Some(id) => conn.execute(
            "UPDATE routes SET layout_id = ?2, name = ?3, block_sequence = ?4, speed_profile = ?5, dwell_times = ?6, auto_return = ?7, enabled = ?8, updated_at = ?9, dirty = 0 WHERE id = ?1",
            params![id, layout, text(row, "name"), sequence_text, speed, dwell, flag(row, "auto_return"), flag(row, "enabled"), updated],
        )?,
    };
    Ok(true)
}

pub fn route_outbound(conn: &Connection, user: &str) -> rusqlite::Result<Vec<(i64, String, Value)>> {
    let mut statement = conn.prepare(
        "SELECT id, sync_id, layout_id, name, block_sequence, speed_profile, dwell_times, auto_return, enabled, created_at, updated_at FROM routes WHERE dirty = 1",
    )?;
    let rows: Vec<_> = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(10)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, Option<String>>(5)?, row.get::<_, Option<String>>(6)?, row.get::<_, bool>(7)?, row.get::<_, bool>(8)?, row.get::<_, String>(9)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for (id, stamp, sync_id, layout, name, sequence, speed, dwell, auto_return, enabled, created) in rows {
        let Some(layout_sync) = sync_id_of(conn, "layouts", layout)? else { continue };
        let mut blocks = Vec::new();
        let local_blocks: Vec<i64> = serde_json::from_str(&sequence).unwrap_or_default();
        let mut complete = true;
        for local in local_blocks {
            match sync_id_of(conn, "blocks", local)? {
                Some(sync) => blocks.push(sync),
                None => complete = false,
            }
        }
        if !complete {
            continue;
        }
        out.push((id, stamp.clone(), json!({
            "id": sync_id, "user_id": user, "layout_id": layout_sync, "name": name, "block_sequence": blocks,
            "speed_profile": speed.and_then(|text| serde_json::from_str::<Value>(&text).ok()),
            "dwell_times": dwell.and_then(|text| serde_json::from_str::<Value>(&text).ok()),
            "auto_return": auto_return, "enabled": enabled,
            "created_at": to_cloud(&created), "updated_at": to_cloud(&stamp), "deleted_at": Value::Null,
        })));
    }
    Ok(out)
}

fn num(row: &Value, key: &str) -> f64 {
    row.get(key).and_then(Value::as_f64).unwrap_or_default()
}

fn int(row: &Value, key: &str) -> i64 {
    row.get(key).and_then(Value::as_i64).unwrap_or_default()
}

fn flag(row: &Value, key: &str) -> bool {
    row.get(key).and_then(Value::as_bool).unwrap_or_default()
}
