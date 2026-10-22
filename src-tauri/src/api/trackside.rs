//! Blocks, signals, feedbacks, and routes. Occupancy and aspects are set by hand until the hardware links exist

use std::sync::Arc;

use serde_json::{json, Value};

use super::{optional_layout_id, Call};
use crate::db::layouts;
use crate::db::trackside::{self, NewBlock, NewRoute};
use crate::fields::{self, Body};
use crate::http::{ApiError, Out, Reply};
use crate::state::AppState;

const BLOCK_STATES: [&str; 3] = ["free", "occupied", "reserved"];
const SIGNAL_TYPES: [&str; 2] = ["main", "warning"];
const SIGNAL_ASPECTS: [&str; 3] = ["stop", "caution", "go"];

fn lock(app: &Arc<AppState>) -> std::sync::MutexGuard<'_, crate::state::Inner> {
    app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn check_layout(db: &rusqlite::Connection, id: i64) -> Result<(), ApiError> {
    layouts::get(db, id)?.map(|_| ()).ok_or_else(|| ApiError::not_found("Layout", id))
}

/// The choices as the API lists them, sorted
fn choices(allowed: &[&str]) -> String {
    let mut sorted = allowed.to_vec();
    sorted.sort_unstable();
    sorted.join(", ")
}

fn check_choice(value: &str, allowed: &[&str], label: &str) -> Result<(), ApiError> {
    if !allowed.contains(&value) {
        return Err(ApiError::bad_request(format!("{label} must be one of: {}", choices(allowed))));
    }
    Ok(())
}

pub fn blocks(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => {
            let inner = lock(app);
            Ok(Reply::json(200, json!(trackside::blocks(&inner.db, optional_layout_id(call)?)?)))
        }
        ("POST", []) => create_block(app, call),
        ("GET", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            let block = trackside::block(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Block", id))?;
            Ok(Reply::json(200, json!(block)))
        }
        ("PATCH", [id]) => update_block(app, super::id_segment(id)?, call),
        ("DELETE", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            trackside::block(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Block", id))?;
            trackside::delete_block(&inner.db, id)?;
            Ok(Reply::empty(204))
        }
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

fn create_block(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let layout_id = fields::required_int(&body, "layout_id")?;
    let name = fields::required_str(&body, "name")?;
    let x = fields::required_float(&body, "x")?;
    let y = fields::required_float(&body, "y")?;
    let max_speed = fields::optional_int(&body, "max_speed")?.unwrap_or(100);
    let length = fields::optional_float(&body, "length")?;

    let inner = lock(app);
    check_layout(&inner.db, layout_id)?;
    let id = trackside::insert_block(
        &inner.db,
        &NewBlock { layout_id, name: &name, x, y, max_speed, length },
    )?;
    let block = trackside::block(&inner.db, id)?.ok_or_else(|| ApiError::internal("Block was not saved"))?;
    Ok(Reply::json(201, json!(block)))
}

fn update_block(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    let changes: Body = fields::changes(&body, &["name", "x", "y", "max_speed", "state"]);
    fields::reject_nulls(&changes)?;
    if let Some(state) = fields::optional_str(&changes, "state")? {
        check_choice(&state, &BLOCK_STATES, "State")?;
    }

    let inner = lock(app);
    let mut block = trackside::block(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Block", id))?;
    if let Some(name) = fields::optional_str(&changes, "name")? {
        block.name = name;
    }
    if let Some(x) = fields::optional_float(&changes, "x")? {
        block.x = x;
    }
    if let Some(y) = fields::optional_float(&changes, "y")? {
        block.y = y;
    }
    if let Some(max_speed) = fields::optional_int(&changes, "max_speed")? {
        block.max_speed = max_speed;
    }
    if let Some(state) = fields::optional_str(&changes, "state")? {
        block.occupied = state == "occupied";
        block.state = state;
    }
    trackside::save_block(&inner.db, &block)?;
    let saved = trackside::block(&inner.db, id)?.ok_or_else(|| ApiError::internal("Block was not saved"))?;
    Ok(Reply::json(200, json!(saved)))
}

pub fn signals(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => {
            let inner = lock(app);
            Ok(Reply::json(200, json!(trackside::signals(&inner.db, optional_layout_id(call)?)?)))
        }
        ("POST", []) => create_signal(app, call),
        ("GET", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            let signal = trackside::signal(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Signal", id))?;
            Ok(Reply::json(200, json!(signal)))
        }
        ("PATCH", [id]) => update_signal(app, super::id_segment(id)?, call),
        ("DELETE", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            trackside::signal(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Signal", id))?;
            trackside::delete_signal(&inner.db, id)?;
            Ok(Reply::empty(204))
        }
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

fn create_signal(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let layout_id = fields::required_int(&body, "layout_id")?;
    let name = fields::required_str(&body, "name")?;
    let x = fields::required_float(&body, "x")?;
    let y = fields::required_float(&body, "y")?;
    let signal_type = fields::required_str(&body, "signal_type")?;
    check_choice(&signal_type, &SIGNAL_TYPES, "Signal type")?;

    let inner = lock(app);
    check_layout(&inner.db, layout_id)?;
    let id = trackside::insert_signal(&inner.db, layout_id, &name, x, y, &signal_type)?;
    let signal = trackside::signal(&inner.db, id)?.ok_or_else(|| ApiError::internal("Signal was not saved"))?;
    Ok(Reply::json(201, json!(signal)))
}

fn update_signal(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    let changes: Body = fields::changes(&body, &["name", "x", "y", "aspect"]);
    fields::reject_nulls(&changes)?;
    if let Some(aspect) = fields::optional_str(&changes, "aspect")? {
        check_choice(&aspect, &SIGNAL_ASPECTS, "Aspect")?;
    }

    let inner = lock(app);
    let mut signal = trackside::signal(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Signal", id))?;
    if let Some(name) = fields::optional_str(&changes, "name")? {
        signal.name = name;
    }
    if let Some(x) = fields::optional_float(&changes, "x")? {
        signal.x = x;
    }
    if let Some(y) = fields::optional_float(&changes, "y")? {
        signal.y = y;
    }
    if let Some(aspect) = fields::optional_str(&changes, "aspect")? {
        signal.aspect = aspect;
    }
    trackside::save_signal(&inner.db, &signal)?;
    let saved = trackside::signal(&inner.db, id)?.ok_or_else(|| ApiError::internal("Signal was not saved"))?;
    Ok(Reply::json(200, json!(saved)))
}

pub fn feedbacks(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => {
            let inner = lock(app);
            Ok(Reply::json(200, json!(trackside::feedbacks(&inner.db, optional_layout_id(call)?)?)))
        }
        ("POST", []) => create_feedback(app, call),
        ("GET", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            let feedback = trackside::feedback(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Feedback", id))?;
            Ok(Reply::json(200, json!(feedback)))
        }
        ("PATCH", [id]) => update_feedback(app, super::id_segment(id)?, call),
        ("DELETE", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            trackside::feedback(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Feedback", id))?;
            trackside::delete_feedback(&inner.db, id)?;
            Ok(Reply::empty(204))
        }
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

fn create_feedback(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let layout_id = fields::required_int(&body, "layout_id")?;
    let name = fields::required_str(&body, "name")?;
    let x = fields::required_float(&body, "x")?;
    let y = fields::required_float(&body, "y")?;
    let number = fields::required_int(&body, "feedback_number")?;

    let inner = lock(app);
    check_layout(&inner.db, layout_id)?;
    let id = trackside::insert_feedback(&inner.db, layout_id, &name, x, y, number)?;
    let feedback = trackside::feedback(&inner.db, id)?.ok_or_else(|| ApiError::internal("Feedback was not saved"))?;
    Ok(Reply::json(201, json!(feedback)))
}

fn update_feedback(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    let changes: Body = fields::changes(&body, &["name", "x", "y", "state"]);
    fields::reject_nulls(&changes)?;

    let inner = lock(app);
    let mut feedback = trackside::feedback(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Feedback", id))?;
    if let Some(name) = fields::optional_str(&changes, "name")? {
        feedback.name = name;
    }
    if let Some(x) = fields::optional_float(&changes, "x")? {
        feedback.x = x;
    }
    if let Some(y) = fields::optional_float(&changes, "y")? {
        feedback.y = y;
    }
    if let Some(state) = fields::optional_bool(&changes, "state")? {
        feedback.state = state;
    }
    trackside::save_feedback(&inner.db, &feedback)?;
    let saved = trackside::feedback(&inner.db, id)?.ok_or_else(|| ApiError::internal("Feedback was not saved"))?;
    Ok(Reply::json(200, json!(saved)))
}

pub fn routes(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => {
            let inner = lock(app);
            Ok(Reply::json(200, json!(trackside::routes(&inner.db, optional_layout_id(call)?)?)))
        }
        ("POST", []) => create_route(app, call),
        ("GET", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            let route = trackside::route(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Route", id))?;
            Ok(Reply::json(200, json!(route)))
        }
        ("PATCH", [id]) => update_route(app, super::id_segment(id)?, call),
        ("DELETE", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            let route = trackside::route(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Route", id))?;
            if route.is_running {
                return Err(ApiError::conflict("Cannot delete a running route"));
            }
            trackside::delete_route(&inner.db, id)?;
            Ok(Reply::empty(204))
        }
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

/// Every block in the sequence must exist in the layout, and there must be at least one
fn check_sequence(db: &rusqlite::Connection, layout_id: i64, sequence: &[i64]) -> Result<(), ApiError> {
    if sequence.is_empty() {
        return Err(ApiError::bad_request("Route needs at least one block"));
    }
    let known: Vec<i64> = trackside::blocks(db, Some(layout_id))?.iter().map(|block| block.id).collect();
    let missing: Vec<i64> = sequence.iter().copied().filter(|id| !known.contains(id)).collect();
    if !missing.is_empty() {
        let listed: Vec<String> = missing.iter().map(i64::to_string).collect();
        return Err(ApiError::bad_request(format!(
            "Blocks not in layout {layout_id}: [{}]",
            listed.join(", ")
        )));
    }
    Ok(())
}

fn sequence_of(value: &Value) -> Result<Vec<i64>, ApiError> {
    value
        .as_array()
        .and_then(|items| items.iter().map(Value::as_i64).collect())
        .ok_or_else(|| ApiError::unprocessable("block_sequence must be a list of block ids"))
}

fn create_route(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let layout_id = fields::required_int(&body, "layout_id")?;
    let name = fields::required_str(&body, "name")?;
    let sequence = body
        .get("block_sequence")
        .map(sequence_of)
        .unwrap_or_else(|| Err(ApiError::unprocessable("block_sequence is required")))?;
    let speed_profile = body.get("speed_profile").filter(|value| !value.is_null()).cloned();
    let dwell_times = body.get("dwell_times").filter(|value| !value.is_null()).cloned();
    let auto_return = fields::optional_bool(&body, "auto_return")?.unwrap_or(false);

    let inner = lock(app);
    check_layout(&inner.db, layout_id)?;
    check_sequence(&inner.db, layout_id, &sequence)?;
    let id = trackside::insert_route(
        &inner.db,
        &NewRoute {
            layout_id,
            name: &name,
            block_sequence: &sequence,
            speed_profile: speed_profile.as_ref(),
            dwell_times: dwell_times.as_ref(),
            auto_return,
        },
    )?;
    let route = trackside::route(&inner.db, id)?.ok_or_else(|| ApiError::internal("Route was not saved"))?;
    Ok(Reply::json(201, json!(route)))
}

fn update_route(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    if body.contains_key("is_running") {
        return Err(ApiError::bad_request("Route execution is not built yet"));
    }
    let changes: Body = fields::changes(&body, &["name", "block_sequence", "speed_profile", "enabled"]);
    fields::reject_nulls(&changes)?;

    let inner = lock(app);
    let mut route = trackside::route(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Route", id))?;
    if let Some(value) = changes.get("block_sequence") {
        let sequence = sequence_of(value)?;
        check_sequence(&inner.db, route.layout_id, &sequence)?;
        route.block_sequence = sequence;
    }
    if let Some(name) = fields::optional_str(&changes, "name")? {
        route.name = name;
    }
    if let Some(profile) = changes.get("speed_profile") {
        route.speed_profile = Some(profile.clone());
    }
    if let Some(enabled) = fields::optional_bool(&changes, "enabled")? {
        route.enabled = enabled;
    }
    trackside::save_route(&inner.db, &route)?;
    let saved = trackside::route(&inner.db, id)?.ok_or_else(|| ApiError::internal("Route was not saved"))?;
    Ok(Reply::json(200, json!(saved)))
}
