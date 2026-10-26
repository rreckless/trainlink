//! Layout endpoints, and the checks on the track pieces saved in a layout

use std::collections::HashSet;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{is_number, Call};
use crate::db::{is_unique_violation, layouts};
use crate::fields::{self, Body};
use crate::http::{ApiError, Out, Reply};
use crate::library::library;
use crate::anyrail;
use crate::state::AppState;

const TRACK_KEYS: [&str; 4] = ["x1", "y1", "x2", "y2"];
const LAYOUT_CHANGES: [&str; 5] = ["name", "description", "width", "height", "elements"];
const MAX_XML_CHARS: usize = 5_000_000;

pub fn route(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => list(app),
        ("POST", []) => create(app, call),
        ("POST", ["anyrail-import"]) => import(call),
        ("GET", [id]) => get(app, super::id_segment(id)?),
        ("PATCH", [id]) => update(app, super::id_segment(id)?, call),
        ("PATCH", [id, "turnouts", placed]) => update_turnout(app, super::id_segment(id)?, placed, call),
        ("DELETE", [id]) => delete(app, super::id_segment(id)?),
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

/// The settings a turnout can have, with their limits
const THROW_MS_RANGE: std::ops::RangeInclusive<i64> = 50..=5000;
const GROUP_MAX: usize = 40;
const NOTES_MAX: usize = 500;

/// Change a turnout's settings on a layout: its switching time, whether it is manual, its position feedback, its group
/// and its notes. Only the settings sent are changed
fn update_turnout(app: &Arc<AppState>, layout_id: i64, placed_id: &str, call: &Call) -> Out {
    let body = call.json_body()?;
    let throw_ms = fields::optional_int(&body, "throw_ms")?;
    if let Some(ms) = throw_ms {
        if !THROW_MS_RANGE.contains(&ms) {
            return Err(ApiError::unprocessable(format!("Switching time must be {} to {} ms", THROW_MS_RANGE.start(), THROW_MS_RANGE.end())));
        }
    }
    let manual = fields::optional_bool(&body, "manual")?;
    let feedback = fields::optional_bool(&body, "feedback")?;
    let group = fields::optional_str(&body, "group")?.map(|text| text.trim().to_string());
    if group.as_ref().is_some_and(|text| text.chars().count() > GROUP_MAX) {
        return Err(ApiError::unprocessable(format!("Group must be at most {GROUP_MAX} characters")));
    }
    let notes = fields::optional_str(&body, "notes")?;
    if notes.as_ref().is_some_and(|text| text.chars().count() > NOTES_MAX) {
        return Err(ApiError::unprocessable(format!("Notes must be at most {NOTES_MAX} characters")));
    }

    let inner = lock(app);
    let mut layout = layouts::get(&inner.db, layout_id)?.ok_or_else(|| ApiError::not_found("Layout", layout_id))?;
    let placed = layout
        .elements
        .get_mut("track_pieces")
        .and_then(Value::as_array_mut)
        .and_then(|pieces| {
            pieces.iter_mut().find(|piece| {
                piece.get("id").and_then(Value::as_str) == Some(placed_id)
                    && piece.get("piece_id").and_then(Value::as_str).and_then(|id| library().get(id)).is_some_and(|part| part.get("type").and_then(Value::as_str) == Some("turnout"))
            })
        })
        .ok_or_else(|| ApiError::not_found_message(format!("Turnout {placed_id} is not on this layout")))?;
    let object = placed.as_object_mut().ok_or_else(|| ApiError::internal("A placed turnout is not an object"))?;
    if let Some(ms) = throw_ms {
        object.insert("throw_ms".into(), json!(ms));
    }
    if let Some(manual) = manual {
        object.insert("manual".into(), json!(manual));
    }
    if let Some(feedback) = feedback {
        object.insert("feedback".into(), json!(feedback));
    }
    if let Some(group) = group {
        object.insert("group".into(), json!(group));
    }
    if let Some(notes) = notes {
        object.insert("notes".into(), json!(notes.trim()));
    }
    let settings = layouts::turnout_settings(placed);
    layouts::save(&inner.db, &layout)?;
    Ok(Reply::json(200, json!({ "layout_id": layout_id, "placed_id": placed_id, "settings": settings })))
}

fn lock(app: &Arc<AppState>) -> std::sync::MutexGuard<'_, crate::state::Inner> {
    app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn list(app: &Arc<AppState>) -> Out {
    let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let rows = layouts::list(&inner.db)?;
    Ok(Reply::json(200, json!(rows)))
}

fn get_layout(app: &Arc<AppState>, id: i64) -> Result<layouts::Layout, ApiError> {
    let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    layouts::get(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Layout", id))
}

fn get(app: &Arc<AppState>, id: i64) -> Out {
    Ok(Reply::json(200, json!(get_layout(app, id)?)))
}

fn check_positive(width: f64, height: f64) -> Result<(), ApiError> {
    if width <= 0.0 || height <= 0.0 {
        return Err(ApiError::bad_request("Width and height must be positive"));
    }
    Ok(())
}

fn create(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let name = fields::required_str(&body, "name")?;
    let width = fields::required_float(&body, "width")?;
    let height = fields::required_float(&body, "height")?;
    let scale = fields::required_str(&body, "scale")?;
    let description = fields::optional_str(&body, "description")?;
    let track_gauge = fields::optional_str(&body, "track_gauge")?;
    check_positive(width, height)?;

    let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let new = layouts::NewLayout {
        name: &name,
        description: description.as_deref(),
        width,
        height,
        scale: &scale,
        track_gauge: track_gauge.as_deref(),
    };
    let id = layouts::insert(&inner.db, &new).map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("A layout with this name already exists")
        } else {
            error.into()
        }
    })?;
    let created = layouts::get(&inner.db, id)?.ok_or_else(|| ApiError::internal("Layout was not saved"))?;
    Ok(Reply::json(201, json!(created)))
}

fn import(call: &Call) -> Out {
    let body = call.json_body()?;
    let xml = fields::required_str(&body, "xml")?;
    let radius = fields::required_float(&body, "inner_radius_mm")?;
    if xml.is_empty() || xml.len() > MAX_XML_CHARS {
        return Err(ApiError::unprocessable("xml must be 1 to 5,000,000 characters"));
    }
    if !(radius > 0.0 && radius <= 10_000.0) {
        return Err(ApiError::unprocessable("inner_radius_mm must be above 0 and at most 10,000"));
    }
    let result = anyrail::import(&xml, radius).map_err(ApiError::bad_request)?;
    check_track_pieces(&json!({
        "track_system": result["track_system"],
        "track_pieces": result["track_pieces"],
    }))?;
    Ok(Reply::json(200, result))
}

fn update(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    let changes: Body = fields::changes(&body, &LAYOUT_CHANGES);
    fields::reject_nulls(&changes)?;

    let size_changes = changes.contains_key("width") || changes.contains_key("height");
    if size_changes {
        let current = get_layout(app, id)?;
        let width = fields::optional_float(&changes, "width")?.unwrap_or(current.width);
        let height = fields::optional_float(&changes, "height")?.unwrap_or(current.height);
        check_positive(width, height)?;
    }
    if let Some(elements) = changes.get("elements") {
        if !elements.is_object() {
            return Err(ApiError::unprocessable("elements must be an object"));
        }
        check_elements(elements)?;
    }
    let name = match fields::optional_str(&changes, "name")? {
        Some(name) => {
            let trimmed = name.trim().to_string();
            if trimmed.is_empty() {
                return Err(ApiError::bad_request("A layout needs a name"));
            }
            Some(trimmed)
        }
        None => None,
    };

    let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut layout = layouts::get(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Layout", id))?;
    if let Some(name) = name {
        layout.name = name;
    }
    if let Some(description) = fields::optional_str(&changes, "description")? {
        layout.description = Some(description);
    }
    if let Some(width) = fields::optional_float(&changes, "width")? {
        layout.width = width;
    }
    if let Some(height) = fields::optional_float(&changes, "height")? {
        layout.height = height;
    }
    if let Some(elements) = changes.get("elements") {
        layout.elements = elements.clone();
    }
    layouts::save(&inner.db, &layout).map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("A layout with this name already exists")
        } else {
            error.into()
        }
    })?;
    let saved = layouts::get(&inner.db, id)?.ok_or_else(|| ApiError::internal("Layout was not saved"))?;
    Ok(Reply::json(200, json!(saved)))
}

fn delete(app: &Arc<AppState>, id: i64) -> Out {
    let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if layouts::get(&inner.db, id)?.is_none() {
        return Err(ApiError::not_found("Layout", id));
    }
    let contents = layouts::contents(&inner.db, id)?;
    if !contents.is_empty() {
        return Err(ApiError::conflict(format!(
            "Layout still has {}. Delete them first",
            contents.join(", ")
        )));
    }
    layouts::delete(&inner.db, id)?;
    Ok(Reply::empty(204))
}

/// Straight track segments, then the placed track pieces
pub fn check_elements(elements: &Value) -> Result<(), ApiError> {
    let tracks = elements.get("tracks").cloned().unwrap_or_else(|| json!([]));
    let Some(tracks) = tracks.as_array() else {
        return Err(ApiError::bad_request("elements.tracks must be a list"));
    };
    for track in tracks {
        let valid = track.is_object() && TRACK_KEYS.iter().all(|key| is_number(track.get(*key)));
        if !valid {
            return Err(ApiError::bad_request(format!("Each track needs numeric {}", TRACK_KEYS.join(", "))));
        }
    }
    check_track_pieces(elements)
}

/// Placed library pieces must exist, share one track system, and match elements.track_system
pub fn check_track_pieces(elements: &Value) -> Result<(), ApiError> {
    let pieces = elements.get("track_pieces").cloned().unwrap_or_else(|| json!([]));
    let Some(pieces) = pieces.as_array() else {
        return Err(ApiError::bad_request("elements.track_pieces must be a list"));
    };
    let library = library();
    let mut systems: HashSet<String> = HashSet::new();
    for placed in pieces {
        let piece_id = placed.get("piece_id").and_then(Value::as_str);
        let valid = placed.is_object()
            && placed.get("id").is_some_and(Value::is_string)
            && piece_id.is_some_and(|id| library.contains(id))
            && ["x", "y", "rotation"].iter().all(|key| is_number(placed.get(*key)))
            && placed.get("flip").is_some_and(Value::is_boolean);
        if !valid {
            return Err(ApiError::bad_request(
                "Each track piece needs an id, a known piece_id, x, y, rotation, and a flip flag",
            ));
        }
        let piece = library.get(piece_id.unwrap_or_default()).cloned().unwrap_or(Value::Null);
        if !valid_dimensions(placed, &piece) {
            return Err(ApiError::bad_request(
                "A placed length, radius, or angle must be a positive number that suits the piece",
            ));
        }
        if !valid_handles(placed, &piece) {
            return Err(ApiError::bad_request(
                "Bend points need two x and y values in mm, and only flex track can have them",
            ));
        }
        if !valid_dcc(placed, &piece) {
            return Err(ApiError::bad_request(
                "A DCC address (1-2048) or straight output (0 or 1) must suit the piece. Only turnouts take them",
            ));
        }
        if let Some(system) = piece["system"].as_str() {
            systems.insert(system.to_string());
        }
    }
    let addresses: Vec<&Value> = pieces.iter().filter_map(|placed| placed.get("dcc_address")).collect();
    let unique: HashSet<String> = addresses.iter().map(|value| value.to_string()).collect();
    if unique.len() != addresses.len() {
        return Err(ApiError::bad_request("A DCC turnout address can only be used once in a layout"));
    }
    if systems.len() > 1 {
        return Err(ApiError::bad_request("A layout cannot mix track systems"));
    }
    if let Some(system) = systems.iter().next() {
        if elements.get("track_system").and_then(Value::as_str) != Some(system.as_str()) {
            return Err(ApiError::bad_request(format!("elements.track_system must be '{system}' for these pieces")));
        }
    }
    Ok(())
}

/// Length, radius, and angle overrides, each only for the piece types that take them
fn valid_dimensions(placed: &Value, piece: &Value) -> bool {
    let kind = piece["type"].as_str().unwrap_or_default();
    let rules: [(&str, &[&str], f64); 3] = [
        ("length", &["straight", "flex"], 100_000.0),
        ("radius", &["curve"], 10_000.0),
        ("angle", &["curve"], 360.0),
    ];
    for (key, kinds, largest) in rules {
        let Some(value) = placed.get(key) else { continue };
        let Some(number) = value.as_f64().filter(|_| value.is_number()) else { return false };
        if !kinds.contains(&kind) || !(number > 0.0 && number <= largest) {
            return false;
        }
    }
    // A curve without a radius in the library needs one on the placement
    !(kind == "curve" && placed.get("radius").is_none() && piece.get("radius").is_none())
}

/// Two curve handles, each with an x and a y in mm, which only flex track may have
fn valid_handles(placed: &Value, piece: &Value) -> bool {
    let Some(handles) = placed.get("handles") else { return true };
    if piece["type"].as_str() != Some("flex") {
        return false;
    }
    let Some(points) = handles.as_array().filter(|points| points.len() == 2) else { return false };
    points.iter().all(|point| {
        ["x", "y"].iter().all(|key| {
            point
                .get(*key)
                .and_then(Value::as_f64)
                .is_some_and(|value| value.abs() <= 10_000.0)
        })
    })
}

/// A DCC address (1-2048) and a straight output (0 or 1), which only turnouts may have
fn valid_dcc(placed: &Value, piece: &Value) -> bool {
    if placed.get("dcc_address").is_none() && placed.get("dcc_output_straight").is_none() {
        return true;
    }
    if piece["type"].as_str() != Some("turnout") {
        return false;
    }
    if let Some(address) = placed.get("dcc_address") {
        match address.as_i64() {
            Some(number) if (1..=2048).contains(&number) => {}
            _ => return false,
        }
    }
    let straight = placed.get("dcc_output_straight").and_then(Value::as_i64).unwrap_or(0);
    placed
        .get("dcc_output_straight")
        .map_or(true, |value| value.as_i64().is_some())
        && (straight == 0 || straight == 1)
}
