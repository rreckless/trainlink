//! Project-wide overviews, read from the saved layouts and blocks. The turnout list shows every turnout on every layout.
//! The health check lists what is missing before automatic driving can work, as LocoLink's project diagnostics does

use std::sync::Arc;

use serde_json::{json, Value};

use super::Call;
use crate::db::{self, layouts as layouts_db};
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
        ("GET", ["log"]) => connection_log(app),
        ("GET", ["support"]) => support(app),
        ("GET", ["setup"]) => setup(app),
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

/// One health category: ok, needs attention, or not built yet
fn category(id: &str, label: &str, ok: bool, detail: String) -> Value {
    let status = if ok { "ok" } else { "attention" };
    json!({ "id": id, "label": label, "status": status, "detail": detail })
}

fn not_built(id: &str, label: &str) -> Value {
    json!({ "id": id, "label": label, "status": "not_built", "detail": "Not built yet" })
}

/// The setup checklist: each step is done when the data for it exists, and names the page where it is done
fn setup(app: &Arc<AppState>) -> Out {
    let inner = lock(app);
    let stations: i64 = inner.db.query_row("SELECT COUNT(*) FROM command_stations", [], |row| row.get(0))?;
    let locomotives = crate::db::locomotives::list(&inner.db)?;
    let moved = locomotives.iter().any(|loco| loco.travelled_time > 0.0);
    let placed = layouts_db::list(&inner.db)?.iter().any(|layout| {
        let pieces = layout.elements.get("track_pieces").and_then(Value::as_array).is_some_and(|items| !items.is_empty());
        let tracks = layout.elements.get("tracks").and_then(Value::as_array).is_some_and(|items| !items.is_empty());
        pieces || tracks
    });
    let blocks: i64 = inner.db.query_row("SELECT COUNT(*) FROM blocks", [], |row| row.get(0))?;
    let blocks_missing: i64 = inner.db.query_row(
        "SELECT COUNT(*) FROM blocks WHERE id NOT IN (SELECT block_id FROM block_feedbacks)",
        [],
        |row| row.get(0),
    )?;
    let turnouts = turnout_rows(&inner)?;
    let turnouts_missing = turnouts.iter().filter(|row| row["has_address"] == Value::Bool(false)).count();
    let routes: i64 = inner.db.query_row("SELECT COUNT(*) FROM routes", [], |row| row.get(0))?;

    let steps = vec![
        step("command_station", "Add a command station", stations > 0, format!("{stations} command station(s)"), "settings"),
        step("locomotives", "Add locomotives", !locomotives.is_empty(), format!("{} locomotive(s)", locomotives.len()), "locomotives"),
        step("first_run", "Make a first manual run", moved, "A locomotive has moved under TrainLink control".into(), "control"),
        step("track_plan", "Draw or import the track plan", placed, "A layout has track on it".into(), "editor"),
        step(
            "blocks",
            "Set up blocks and sensors",
            blocks > 0 && blocks_missing == 0,
            format!("{blocks} block(s), {blocks_missing} without a sensor"),
            "blocks",
        ),
        step(
            "turnouts",
            "Give every turnout a DCC address",
            !turnouts.is_empty() && turnouts_missing == 0,
            format!("{} turnout(s), {turnouts_missing} without an address", turnouts.len()),
            "turnouts",
        ),
        step("routes", "Create routes", routes > 0, format!("{routes} route(s)"), "routes"),
    ];
    let done = steps.iter().filter(|item| item["done"] == Value::Bool(true)).count();
    Ok(Reply::json(200, json!({ "steps": steps, "done": done, "total": steps.len() })))
}

fn step(id: &str, label: &str, done: bool, detail: String, page: &str) -> Value {
    json!({ "id": id, "label": label, "done": done, "detail": detail, "page": page })
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
    Ok(Reply::json(200, health_report(&inner)?))
}

/// The health report: what is missing, the counts, and the score and categories. Shared with the support package
fn health_report(inner: &Inner) -> Result<Value, ApiError> {
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
    // Legs found by the AnyRail import are kept in the layout's elements until the user clears them
    let mut legs: Vec<Value> = Vec::new();
    for layout in crate::db::layouts::list(&inner.db)? {
        for item in layout.elements.get("no_track_legs").and_then(Value::as_array).into_iter().flatten() {
            legs.push(json!({
                "layout_id": layout.id,
                "layout": layout.name,
                "turnout": item["turnout"],
                "leg": item["leg"],
                "point": item["point"],
            }));
        }
    }
    let blocks_total: i64 = inner.db.query_row("SELECT COUNT(*) FROM blocks", [], |row| row.get(0))?;
    let turnouts_total = turnout_rows(&inner)?.len();
    let stations: i64 = inner.db.query_row("SELECT COUNT(*) FROM command_stations", [], |row| row.get(0))?;
    let locomotives: i64 = inner.db.query_row("SELECT COUNT(*) FROM locomotives", [], |row| row.get(0))?;
    let routes: i64 = inner.db.query_row("SELECT COUNT(*) FROM routes", [], |row| row.get(0))?;
    let categories = vec![
        category("command_stations", "Command stations", stations > 0, format!("{stations} command station(s)")),
        category("locomotives", "Locomotives", locomotives > 0, format!("{locomotives} locomotive(s)")),
        category("track", "Track", legs.is_empty(), format!("{} turnout leg(s) with no track", legs.len())),
        category("turnouts", "Turnouts", turnouts.is_empty(), format!("{} without an address", turnouts.len())),
        category("blocks", "Blocks", blocks.is_empty(), format!("{} without a sensor", blocks.len())),
        category("routes", "Routes", routes > 0, format!("{routes} route(s)")),
        not_built("cameras", "Cameras"),
        not_built("recovery", "Recovery"),
    ];
    // Categories that are not built yet are left out of the score, so they cannot lower it
    let scored: Vec<&Value> = categories.iter().filter(|item| item["status"] != "not_built").collect();
    let fine = scored.iter().filter(|item| item["status"] == "ok").count();
    let score = if scored.is_empty() { 0 } else { fine * 100 / scored.len() };
    Ok(json!({
        "blocks_without_sensor": blocks,
        "turnouts_without_address": turnouts,
        "legs_without_track": legs,
        "counts": {
            "blocks": blocks_total,
            "turnouts": turnouts_total,
            "missing": blocks.len() + turnouts.len() + legs.len(),
        },
        "score": score,
        "categories": categories,
    }))
}

/// The connection log, oldest first
fn connection_log(app: &Arc<AppState>) -> Out {
    let inner = lock(app);
    let entries: Vec<Value> = inner.z21.log.iter().map(|entry| json!({ "at": entry.at, "message": entry.message })).collect();
    Ok(Reply::json(200, json!(entries)))
}

/// A plain text package to send with a support request. It has counts, statuses and the recent log, and leaves out
/// names, addresses and the IP address of the Z21
fn support(app: &Arc<AppState>) -> Out {
    let inner = lock(app);
    let report = health_report(&inner)?;
    let layouts: i64 = inner.db.query_row("SELECT COUNT(*) FROM layouts", [], |row| row.get(0))?;
    let locomotives: i64 = inner.db.query_row("SELECT COUNT(*) FROM locomotives", [], |row| row.get(0))?;
    let z21 = &inner.z21;
    let mut lines = vec![
        "TrainLink support package".to_string(),
        "Names, addresses and the IP address of the Z21 are left out.".to_string(),
        format!("Created: {}", db::now()),
        format!("App version: {}", env!("CARGO_PKG_VERSION")),
        format!("Z21 connected: {}", if z21.connected { "yes" } else { "no" }),
        format!("Track state: {}", z21.track_state()),
    ];
    if let Some(hardware) = &z21.hardware {
        let model = hardware.model().unwrap_or("unknown model");
        lines.push(format!("Z21 hardware: {model}, firmware {}", hardware.firmware_text()));
    }
    if let Some(version) = &z21.version {
        lines.push(format!("X-Bus version: {}, station ID {}", version.xbus, version.station_id));
    }
    lines.push(format!("Layouts: {layouts}, locomotives: {locomotives}, blocks: {}", report["counts"]["blocks"]));
    lines.push(format!("Health score: {}%", report["score"]));
    for category in report["categories"].as_array().into_iter().flatten() {
        lines.push(format!("  {}: {} ({})", category["label"].as_str().unwrap_or_default(), category["status"].as_str().unwrap_or_default(), category["detail"].as_str().unwrap_or_default()));
    }
    lines.push("Recent connection log:".to_string());
    let recent: Vec<_> = z21.log.iter().rev().take(100).collect();
    for entry in recent.iter().rev() {
        lines.push(format!("  {} {}", entry.at, entry.message));
    }
    Ok(Reply::json(200, json!({ "text": lines.join("\n") })))
}
