//! Blocks, signals, feedbacks, and routes. Occupancy and aspects are set by hand until the hardware links exist

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{optional_layout_id, Call};
use crate::db::layouts;
use crate::db::trackside::{self, NewBlock, NewRoute};
use crate::fields::{self, Body};
use crate::http::{ApiError, Out, Reply};
use crate::occupancy;
use crate::state::{AppState, Inner};

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
            Ok(Reply::json(200, json!(live_blocks(&inner, trackside::blocks(&inner.db, optional_layout_id(call)?)?)?)))
        }
        ("POST", []) => create_block(app, call),
        ("GET", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            let block = trackside::block(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Block", id))?;
            Ok(Reply::json(200, json!(live_blocks(&inner, vec![block])?.remove(0))))
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
    let changes: Body = fields::changes(&body, &["name", "x", "y", "max_speed", "state", "feedback_ids", "signal_ids"]);
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
    if let Some(next) = body.get("next_block_id") {
        block.next_block_id = match next {
            Value::Null => None,
            value => {
                let next_id = value.as_i64().ok_or_else(|| ApiError::unprocessable("next_block_id must be a whole number or null"))?;
                if next_id == id {
                    return Err(ApiError::bad_request("A block cannot be its own next block"));
                }
                let next_block = trackside::block(&inner.db, next_id)?.ok_or_else(|| ApiError::not_found("Block", next_id))?;
                if next_block.layout_id != block.layout_id {
                    return Err(ApiError::bad_request("The next block must be on the same layout"));
                }
                Some(next_id)
            }
        };
    }
    if let Some(feedback_ids) = int_list(&changes, "feedback_ids")? {
        check_feedbacks(&inner.db, &feedback_ids, block.layout_id)?;
        trackside::set_block_feedbacks(&inner.db, id, &feedback_ids)?;
    }
    if let Some(signal_ids) = int_list(&changes, "signal_ids")? {
        check_signals(&inner.db, &signal_ids, block.layout_id)?;
        trackside::set_block_signals(&inner.db, id, &signal_ids)?;
    }
    trackside::save_block(&inner.db, &block)?;
    let saved = trackside::block(&inner.db, id)?.ok_or_else(|| ApiError::internal("Block was not saved"))?;
    Ok(Reply::json(200, json!(live_blocks(&inner, vec![saved])?.remove(0))))
}

pub fn signals(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => {
            let inner = lock(app);
            Ok(Reply::json(200, json!(live_signals(&inner, trackside::signals(&inner.db, optional_layout_id(call)?)?)?)))
        }
        ("POST", []) => create_signal(app, call),
        ("GET", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            let signal = trackside::signal(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Signal", id))?;
            Ok(Reply::json(200, json!(live_signals(&inner, vec![signal])?.remove(0))))
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
    let changes: Body = fields::changes(&body, &["name", "x", "y", "aspect", "block_ids"]);
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
    let block_ids = int_list(&changes, "block_ids")?;
    let linked = match &block_ids {
        Some(ids) => !ids.is_empty(),
        None => !signal.block_ids.is_empty(),
    };
    if changes.contains_key("aspect") && linked {
        return Err(ApiError::conflict("This signal's aspect follows the blocks it protects. Unlink the blocks to set it by hand"));
    }
    if let Some(aspect) = fields::optional_str(&changes, "aspect")? {
        signal.aspect = aspect;
    }
    if let Some(block_ids) = block_ids {
        check_blocks(&inner.db, &block_ids, signal.layout_id)?;
        trackside::set_signal_blocks(&inner.db, id, &block_ids)?;
        signal.block_ids = block_ids;
    }
    trackside::save_signal(&inner.db, &signal)?;
    let saved = trackside::signal(&inner.db, id)?.ok_or_else(|| ApiError::internal("Signal was not saved"))?;
    Ok(Reply::json(200, json!(live_signals(&inner, vec![saved])?.remove(0))))
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
            let routes = trackside::routes(&inner.db, optional_layout_id(call)?)?;
            let values = routes.iter().map(|route| route_value(&inner.db, route)).collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(Reply::json(200, json!(values)))
        }
        ("POST", []) => create_route(app, call),
        ("GET", [id]) => {
            let id = super::id_segment(id)?;
            let inner = lock(app);
            let route = trackside::route(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Route", id))?;
            Ok(Reply::json(200, route_value(&inner.db, &route)?))
        }
        ("PATCH", [id]) => update_route(app, super::id_segment(id)?, call),
        ("POST", [id, "run"]) => run_route(app, super::id_segment(id)?, call),
        ("POST", [id, "stop"]) => stop_route(app, super::id_segment(id)?),
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
    if sequence.iter().enumerate().any(|(index, id)| sequence[..index].contains(id)) {
        return Err(ApiError::unprocessable("A block can appear only once in a route"));
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

/// The steps of a route that do not follow the block's own next-block link, as [from, to] pairs. A route is still
/// allowed to have them, but the Route Manager shows them, so the route is checked before it is run
fn unlinked_steps(db: &rusqlite::Connection, sequence: &[i64]) -> rusqlite::Result<Vec<[i64; 2]>> {
    let mut steps = Vec::new();
    for pair in sequence.windows(2) {
        let next: Option<i64> = db
            .query_row("SELECT next_block_id FROM blocks WHERE id = ?1", [pair[0]], |row| row.get(0))
            .unwrap_or(None);
        if next != Some(pair[1]) {
            steps.push([pair[0], pair[1]]);
        }
    }
    Ok(steps)
}

/// A route as the API shows it: the stored fields, and the steps that are not linked
fn route_value(db: &rusqlite::Connection, route: &trackside::Route) -> rusqlite::Result<Value> {
    let mut value = json!(route);
    value["unlinked_steps"] = json!(unlinked_steps(db, &route.block_sequence)?);
    Ok(value)
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
    let speed_profile = step_profile(body.get("speed_profile"), sequence.len(), "speed_profile")?;
    let dwell_times = step_profile(body.get("dwell_times"), sequence.len(), "dwell_times")?;
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
    Ok(Reply::json(201, route_value(&inner.db, &route)?))
}

/// Start a route with a locomotive, which is in the route's first block. The Z21 must be ready
fn run_route(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    let locomotive_id = fields::required_int(&body, "locomotive_id")?;
    let mut inner = lock(app);
    super::z21::ready_check(&inner, "Route")?;
    crate::automation::start(&mut inner, id, locomotive_id, std::time::Instant::now())?;
    let route = trackside::route(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Route", id))?;
    Ok(Reply::json(200, route_value(&inner.db, &route)?))
}

fn stop_route(app: &Arc<AppState>, id: i64) -> Out {
    let mut inner = lock(app);
    crate::automation::stop(&mut inner, id, std::time::Instant::now())?;
    let route = trackside::route(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Route", id))?;
    Ok(Reply::json(200, route_value(&inner.db, &route)?))
}

/// A list with one entry per block of a route: each entry is null, meaning the block's own setting, or a value
/// within the range. A null list means the route has none
fn step_profile(value: Option<&Value>, len: usize, key: &str) -> Result<Option<Value>, ApiError> {
    let Some(value) = value.filter(|value| !value.is_null()) else { return Ok(None) };
    let items = value.as_array().ok_or_else(|| ApiError::unprocessable(format!("{key} must be a list, one entry per block")))?;
    if items.len() != len {
        return Err(ApiError::unprocessable(format!("{key} needs one entry per block ({len})")));
    }
    for item in items {
        let valid = match key {
            "speed_profile" => item.is_null() || item.as_i64().is_some_and(|speed| (0..=126).contains(&speed)),
            _ => item.is_null() || item.as_f64().is_some_and(|seconds| (0.0..=3600.0).contains(&seconds)),
        };
        if !valid {
            let range = if key == "speed_profile" { "a speed from 0 to 126" } else { "seconds from 0 to 3600" };
            return Err(ApiError::unprocessable(format!("Each {key} entry must be null or {range}")));
        }
    }
    Ok(Some(value.clone()))
}

fn update_route(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    if body.contains_key("is_running") {
        return Err(ApiError::bad_request("Start and stop a route with its run endpoints"));
    }
    let changes: Body = fields::changes(&body, &["name", "block_sequence", "speed_profile", "dwell_times", "auto_return", "enabled"]);
    fields::reject_nulls(&changes)?;

    let inner = lock(app);
    let mut route = trackside::route(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Route", id))?;
    let editing_plan = ["block_sequence", "speed_profile", "dwell_times", "auto_return"].iter().any(|key| changes.contains_key(*key));
    if route.is_running && editing_plan {
        return Err(ApiError::conflict("Stop the route before changing its blocks or options"));
    }
    let sequence = match changes.get("block_sequence") {
        Some(value) => {
            let sequence = sequence_of(value)?;
            check_sequence(&inner.db, route.layout_id, &sequence)?;
            route.block_sequence = sequence.clone();
            sequence
        }
        None => route.block_sequence.clone(),
    };
    if let Some(profile) = changes.get("speed_profile") {
        route.speed_profile = step_profile(Some(profile), sequence.len(), "speed_profile")?;
    }
    if let Some(times) = changes.get("dwell_times") {
        route.dwell_times = step_profile(Some(times), sequence.len(), "dwell_times")?;
    }
    if let Some(auto_return) = fields::optional_bool(&changes, "auto_return")? {
        route.auto_return = auto_return;
    }
    if let Some(name) = fields::optional_str(&changes, "name")? {
        route.name = name;
    }
    if let Some(enabled) = fields::optional_bool(&changes, "enabled")? {
        route.enabled = enabled;
    }
    trackside::save_route(&inner.db, &route)?;
    let saved = trackside::route(&inner.db, id)?.ok_or_else(|| ApiError::internal("Route was not saved"))?;
    Ok(Reply::json(200, route_value(&inner.db, &saved)?))
}

/// A list of whole-number ids from a request, or None if the request did not give the list
fn int_list(changes: &Body, key: &str) -> Result<Option<Vec<i64>>, ApiError> {
    let Some(value) = changes.get(key) else { return Ok(None) };
    let items = value.as_array().ok_or_else(|| ApiError::unprocessable(format!("{key} must be a list of ids")))?;
    items
        .iter()
        .map(|item| item.as_i64().ok_or_else(|| ApiError::unprocessable(format!("{key} must be whole numbers"))))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// Each id must be a feedback on the same layout as the block that uses it
fn check_feedbacks(db: &rusqlite::Connection, ids: &[i64], layout_id: i64) -> Result<(), ApiError> {
    for id in ids {
        let feedback = trackside::feedback(db, *id)?.ok_or_else(|| ApiError::not_found("Feedback", *id))?;
        if feedback.layout_id != layout_id {
            return Err(ApiError::bad_request("A feedback must be on the same layout as its block"));
        }
    }
    Ok(())
}

/// Each id must be a signal on the same layout as the block that uses it
fn check_signals(db: &rusqlite::Connection, ids: &[i64], layout_id: i64) -> Result<(), ApiError> {
    for id in ids {
        let signal = trackside::signal(db, *id)?.ok_or_else(|| ApiError::not_found("Signal", *id))?;
        if signal.layout_id != layout_id {
            return Err(ApiError::bad_request("A signal must be on the same layout as its block"));
        }
    }
    Ok(())
}

/// Each id must be a block on the same layout as the signal that protects it
fn check_blocks(db: &rusqlite::Connection, ids: &[i64], layout_id: i64) -> Result<(), ApiError> {
    for id in ids {
        let block = trackside::block(db, *id)?.ok_or_else(|| ApiError::not_found("Block", *id))?;
        if block.layout_id != layout_id {
            return Err(ApiError::bad_request("A block must be on the same layout as its signal"));
        }
    }
    Ok(())
}

/// Whether each block is occupied now: its detected state, or its own state where nothing is reported
fn occupied_map(inner: &Inner) -> rusqlite::Result<HashMap<i64, bool>> {
    let detected = occupancy::detected(&inner.db, &inner.z21.feedback)?;
    let mut map = HashMap::new();
    for block in trackside::blocks(&inner.db, None)? {
        map.insert(block.id, detected.get(&block.id).copied().unwrap_or(block.occupied));
    }
    Ok(map)
}

/// The block after each block, where it is known
fn next_map(inner: &Inner) -> rusqlite::Result<HashMap<i64, i64>> {
    Ok(trackside::blocks(&inner.db, None)?
        .into_iter()
        .filter_map(|block| block.next_block_id.map(|next| (block.id, next)))
        .collect())
}

/// Blocks with their detected occupancy (null where no feedback has reported) and whether they are occupied now
fn live_blocks(inner: &Inner, blocks: Vec<trackside::Block>) -> rusqlite::Result<Vec<Value>> {
    let detected = occupancy::detected(&inner.db, &inner.z21.feedback)?;
    Ok(blocks
        .into_iter()
        .map(|block| {
            let found = detected.get(&block.id).copied();
            let mut value = json!(block);
            value["detected"] = json!(found);
            value["occupied_now"] = json!(found.unwrap_or(block.occupied));
            value
        })
        .collect())
}

/// Signals with the aspect they show. A signal that protects blocks shows stop or go from them; one without shows the
/// aspect it was set to
fn live_signals(inner: &Inner, signals: Vec<trackside::Signal>) -> rusqlite::Result<Vec<Value>> {
    let occupied = occupied_map(inner)?;
    let next = next_map(inner)?;
    Ok(signals
        .into_iter()
        .map(|signal| {
            let mut value = json!(signal);
            if signal.block_ids.is_empty() {
                value["aspect_source"] = json!("manual");
            } else {
                let protected: Vec<occupancy::Protected> = signal
                    .block_ids
                    .iter()
                    .map(|id| occupancy::Protected {
                        occupied: occupied.get(id).copied().unwrap_or(false),
                        next_occupied: next.get(id).and_then(|next_id| occupied.get(next_id)).copied().unwrap_or(false),
                    })
                    .collect();
                value["aspect_source"] = json!("blocks");
                value["aspect"] = json!(occupancy::signal_aspect(&signal.signal_type, &protected));
            }
            value
        })
        .collect())
}
