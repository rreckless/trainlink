//! Project-wide overviews, read from the saved layouts and blocks. The turnout list shows every turnout on every layout.
//! The health check lists what is missing before automatic driving can work, as LocoLink's project diagnostics does

use std::sync::Arc;

use serde_json::{json, Value};

use super::Call;
use crate::db::layouts as layouts_db;
use crate::http::{ApiError, Out, Reply};
use crate::library::library;
use crate::state::{AppState, Inner};

fn lock(app: &Arc<AppState>) -> std::sync::MutexGuard<'_, Inner> {
    app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn route(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", ["turnouts"]) => turnouts(app),
        ("GET", ["health"]) => health(app),
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

/// Every turnout placed on every layout, with its DCC address if it has one
fn turnout_rows(inner: &Inner) -> Result<Vec<Value>, ApiError> {
    let mut rows = Vec::new();
    for layout in layouts_db::list(&inner.db)? {
        let pieces = layout.elements.get("track_pieces").and_then(Value::as_array).cloned().unwrap_or_default();
        for placed in pieces {
            let Some(piece_id) = placed.get("piece_id").and_then(Value::as_str) else { continue };
            let Some(piece) = library().get(piece_id) else { continue };
            if piece.get("type").and_then(Value::as_str) != Some("turnout") {
                continue;
            }
            let address = placed.get("dcc_address").and_then(Value::as_i64);
            let straight = placed.get("dcc_output_straight").and_then(Value::as_i64);
            let settings = layouts_db::turnout_settings(&placed);
            // The position the Z21 reported, shown only for a turnout with position feedback on
            let position = match (address, settings["feedback"].as_bool()) {
                (Some(address), Some(true)) => inner.z21.turnouts.get(&address).copied().flatten(),
                _ => None,
            };
            rows.push(json!({
                "layout_id": layout.id,
                "layout_name": layout.name,
                "placed_id": placed.get("id").and_then(Value::as_str),
                "piece_id": piece_id,
                "name": piece.get("name").and_then(Value::as_str).unwrap_or(piece_id),
                "dcc_address": address,
                "dcc_output_straight": straight,
                "has_address": address.is_some(),
                "settings": settings,
                "position": position,
            }));
        }
    }
    Ok(rows)
}

fn turnouts(app: &Arc<AppState>) -> Out {
    let inner = lock(app);
    Ok(Reply::json(200, json!(turnout_rows(&inner)?)))
}

fn health(app: &Arc<AppState>) -> Out {
    let inner = lock(app);
    let blocks: Vec<Value> = {
        let mut statement = inner.db.prepare(
            "SELECT id, name FROM blocks WHERE id NOT IN (SELECT block_id FROM block_feedbacks) ORDER BY name",
        )?;
        let rows = statement.query_map([], |row| Ok(json!({ "id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)? })))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let turnouts: Vec<Value> = turnout_rows(&inner)?
        .into_iter()
        .filter(|row| row["has_address"] == Value::Bool(false) && row["dcc_output_straight"].is_null())
        .collect();
    let blocks_total: i64 = inner.db.query_row("SELECT COUNT(*) FROM blocks", [], |row| row.get(0))?;
    let turnouts_total = turnout_rows(&inner)?.len();
    Ok(Reply::json(
        200,
        json!({
            "blocks_without_sensor": blocks,
            "turnouts_without_address": turnouts,
            "counts": {
                "blocks": blocks_total,
                "turnouts": turnouts_total,
                "missing": blocks.len() + turnouts.len(),
            },
        }),
    ))
}
