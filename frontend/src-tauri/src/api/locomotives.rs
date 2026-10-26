//! Locomotive roster endpoints and their pictures. Driving a locomotive goes through the Z21 endpoints

use std::fs;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{multipart, Call};
use crate::db::locomotives::{self, Locomotive, NewLocomotive, FUNCTION_BUTTONS, LOCOMOTIVE_TYPES, MAX_SPEED_STEP};
use crate::db::is_unique_violation;
use crate::fields::{self, Body};
use crate::http::{ApiError, Out, Reply};
use crate::state::AppState;

const CHANGES: [&str; 9] = ["name", "dcc_address", "locomotive_type", "length", "speed", "direction", "functions", "function_labels", "reverse_direction"];
const IMAGE_MAX_BYTES: usize = 16 * 1024 * 1024;
const FUNCTION_LABEL_MAX: usize = 24;
const MIN_ADDRESS: i64 = 1;
const MAX_ADDRESS: i64 = 9999;

/// Content type, file extension, and the bytes every file of that type starts with
const IMAGE_TYPES: [(&str, &str, &[u8]); 3] = [
    ("image/jpeg", "jpg", b"\xff\xd8\xff"),
    ("image/png", "png", b"\x89PNG\r\n\x1a\n"),
    ("image/webp", "webp", b"RIFF"),
];

pub fn route(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => list(app),
        ("GET", ["next-address"]) => next_address(app, call),
        ("POST", []) => create(app, call),
        ("GET", [id]) => get(app, super::id_segment(id)?),
        ("PATCH", [id]) => update(app, super::id_segment(id)?, call),
        ("DELETE", [id]) => delete(app, super::id_segment(id)?),
        ("POST", [id, "image"]) => upload_image(app, super::id_segment(id)?, call),
        ("GET", [id, "image"]) => get_image(app, super::id_segment(id)?),
        ("DELETE", [id, "image"]) => delete_image(app, super::id_segment(id)?),
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

fn check_type(kind: &str) -> Result<(), ApiError> {
    if !LOCOMOTIVE_TYPES.contains(&kind) {
        return Err(ApiError::unprocessable(format!("Locomotive type must be one of: {}", LOCOMOTIVE_TYPES.join(", "))));
    }
    Ok(())
}

fn check_max_speed(speed: Option<i64>) -> Result<(), ApiError> {
    match speed {
        Some(step) if !(1..=MAX_SPEED_STEP).contains(&step) => {
            Err(ApiError::unprocessable(format!("Maximum speed must be a speed step from 1 to {MAX_SPEED_STEP}")))
        }
        _ => Ok(()),
    }
}

/// A colour as #rrggbb, lower case. Empty text is no colour
fn check_colour(text: &str) -> Result<String, ApiError> {
    let hex = text.strip_prefix('#').filter(|hex| hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()));
    hex.map(|hex| format!("#{}", hex.to_ascii_lowercase()))
        .ok_or_else(|| ApiError::unprocessable("Colour must be a hex colour such as #1e40af"))
}

/// A text field that can be cleared with null. Absent leaves it alone (None), null clears it (Some(None)), and blank text
/// also clears it
fn clearable_text(body: &Body, key: &str) -> Result<Option<Option<String>>, ApiError> {
    match body.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(value) => {
            let text = value.as_str().ok_or_else(|| ApiError::unprocessable(format!("{key} must be text or null")))?.trim();
            Ok(Some((!text.is_empty()).then(|| text.to_string())))
        }
    }
}

/// A number field that can be cleared with null, read the same way as clearable_text
fn clearable_int(body: &Body, key: &str) -> Result<Option<Option<i64>>, ApiError> {
    match body.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(value) => {
            let number = value.as_i64().ok_or_else(|| ApiError::unprocessable(format!("{key} must be a whole number or null")))?;
            Ok(Some(Some(number)))
        }
    }
}

fn check_address(address: i64) -> Result<(), ApiError> {
    if !(MIN_ADDRESS..=MAX_ADDRESS).contains(&address) {
        return Err(ApiError::bad_request(format!("DCC address must be {MIN_ADDRESS}-{MAX_ADDRESS}")));
    }
    Ok(())
}

/// The lowest DCC address no locomotive on the given command station uses, for a new locomotive to take
fn next_address(app: &Arc<AppState>, call: &Call) -> Out {
    let station = call
        .param("command_station_id")
        .map(|value| value.parse::<i64>())
        .transpose()
        .map_err(|_| ApiError::unprocessable("command_station_id must be a whole number"))?;
    let inner = lock(app);
    Ok(Reply::json(200, json!({ "dcc_address": locomotives::next_free_address(&inner.db, station)? })))
}

/// Refuse an address another locomotive on the same command station already uses
fn check_address_free(conn: &rusqlite::Connection, address: i64, command_station_id: Option<i64>, except: Option<i64>) -> Result<(), ApiError> {
    match locomotives::address_user(conn, address, command_station_id, except)? {
        Some(name) => Err(ApiError::conflict(format!("DCC address {address} is already used by {name} on this command station"))),
        None => Ok(()),
    }
}

fn list(app: &Arc<AppState>) -> Out {
    let inner = lock(app);
    Ok(Reply::json(200, json!(locomotives::list(&inner.db)?)))
}

fn lock(app: &Arc<AppState>) -> std::sync::MutexGuard<'_, crate::state::Inner> {
    app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn load(db: &rusqlite::Connection, id: i64) -> Result<Locomotive, ApiError> {
    locomotives::get(db, id)?.ok_or_else(|| ApiError::not_found("Locomotive", id))
}

fn create(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let name = fields::required_str(&body, "name")?;
    let address = fields::required_int(&body, "dcc_address")?;
    let kind = fields::required_str(&body, "locomotive_type")?;
    let command_station_id = fields::optional_int(&body, "command_station_id")?;
    let length = fields::optional_float(&body, "length")?;
    let group = clearable_text(&body, "group_name")?.flatten();
    let max_speed = clearable_int(&body, "max_speed")?.flatten();
    let reverse = fields::optional_bool(&body, "reverse_direction")?.unwrap_or(false);
    let colour = clearable_text(&body, "colour")?.flatten().map(|text| check_colour(&text)).transpose()?;
    check_address(address)?;
    check_type(&kind)?;
    check_max_speed(max_speed)?;

    let inner = lock(app);
    check_address_free(&inner.db, address, command_station_id, None)?;
    if let Some(station) = command_station_id {
        let found: bool = inner
            .db
            .query_row("SELECT COUNT(*) FROM command_stations WHERE id = ?1", [station], |row| row.get::<_, i64>(0))?
            > 0;
        if !found {
            return Err(ApiError::not_found("Command station", station));
        }
    }
    let new = NewLocomotive {
        name: &name,
        dcc_address: address,
        locomotive_type: &kind,
        length,
        command_station_id,
        group_name: group.as_deref(),
        max_speed,
        reverse_direction: reverse,
        colour: colour.as_deref(),
    };
    let id = locomotives::insert(&inner.db, &new).map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("Locomotive name or DCC address already exists")
        } else {
            error.into()
        }
    })?;
    Ok(Reply::json(201, json!(load(&inner.db, id)?)))
}

fn get(app: &Arc<AppState>, id: i64) -> Out {
    let inner = lock(app);
    Ok(Reply::json(200, json!(load(&inner.db, id)?)))
}

/// A function label list: exactly one label per function button
fn parse_labels(value: &Value) -> Result<Value, ApiError> {
    let items = value
        .as_array()
        .ok_or_else(|| ApiError::unprocessable("function_labels must be a list"))?;
    if items.len() != FUNCTION_BUTTONS {
        return Err(ApiError::unprocessable(format!(
            "Give a label for each of the {FUNCTION_BUTTONS} function buttons"
        )));
    }
    let mut labels = Vec::with_capacity(items.len());
    for item in items {
        let text = item
            .get("label")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::unprocessable("Each function label needs a label"))?;
        if text.chars().count() > FUNCTION_LABEL_MAX {
            return Err(ApiError::unprocessable(format!("A function label can be at most {FUNCTION_LABEL_MAX} characters")));
        }
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(ApiError::unprocessable("A function label cannot be blank"));
        }
        let momentary = item.get("momentary").and_then(Value::as_bool).unwrap_or(false);
        labels.push(json!({ "label": trimmed, "momentary": momentary }));
    }
    Ok(Value::Array(labels))
}

fn update(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    let changes: Body = fields::changes(&body, &CHANGES);
    if changes.contains_key("speed") || changes.contains_key("direction") {
        return Err(ApiError::bad_request("Speed and direction are set through /api/z21/loco/speed"));
    }
    fields::reject_nulls(&changes)?;
    if let Some(address) = fields::optional_int(&changes, "dcc_address")? {
        check_address(address)?;
    }
    let name = fields::optional_str(&changes, "name")?;
    let kind = fields::optional_str(&changes, "locomotive_type")?;
    let length = fields::optional_float(&changes, "length")?;
    let functions = fields::optional_int(&changes, "functions")?;
    let address = fields::optional_int(&changes, "dcc_address")?;
    let labels = changes.get("function_labels").map(parse_labels).transpose()?;
    let reverse = fields::optional_bool(&changes, "reverse_direction")?;
    let group = clearable_text(&body, "group_name")?;
    let max_speed = clearable_int(&body, "max_speed")?;
    let colour = clearable_text(&body, "colour")?;
    if let Some(kind) = &kind {
        check_type(kind)?;
    }
    if let Some(Some(step)) = max_speed {
        check_max_speed(Some(step))?;
    }
    let colour = match colour {
        None => None,
        Some(None) => Some(None),
        Some(Some(text)) => Some(Some(check_colour(&text)?)),
    };
    // The profile is read from the body, not the changes, because null clears it
    let profile = match body.get("decoder_profile_id") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(value) => {
            let id = value.as_i64().ok_or_else(|| ApiError::unprocessable("decoder_profile_id must be a whole number or null"))?;
            Some(Some(id))
        }
    };

    let inner = lock(app);
    let mut locomotive = load(&inner.db, id)?;
    if let Some(name) = name {
        locomotive.name = name;
    }
    if let Some(address) = address {
        locomotive.dcc_address = address;
    }
    if let Some(kind) = kind {
        locomotive.locomotive_type = kind;
    }
    if length.is_some() {
        locomotive.length = length;
    }
    if let Some(reverse) = reverse {
        locomotive.reverse_direction = reverse;
    }
    if let Some(group) = group {
        locomotive.group_name = group;
    }
    if let Some(max_speed) = max_speed {
        locomotive.max_speed = max_speed;
    }
    if let Some(colour) = colour {
        locomotive.colour = colour;
    }
    if let Some(functions) = functions {
        locomotive.functions = functions;
    }
    let labels_given = labels.is_some();
    if let Some(labels) = labels {
        locomotive.function_labels = labels;
    }
    if let Some(profile) = profile {
        if let Some(id) = profile {
            let chosen = crate::db::decoder_profiles::get(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Decoder profile", id))?;
            // Named functions come from the profile, unless the labels were set in this change or already customised
            if !labels_given && locomotive.function_labels == locomotives::default_labels() {
                locomotive.function_labels = locomotives::labels_from_profile(&chosen.functions);
            }
        }
        locomotive.decoder_profile_id = profile;
    }
    check_address_free(&inner.db, locomotive.dcc_address, locomotive.command_station_id, Some(id))?;
    locomotives::save(&inner.db, &locomotive).map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("Another locomotive already has this name or DCC address")
        } else {
            error.into()
        }
    })?;
    Ok(Reply::json(200, json!(load(&inner.db, id)?)))
}

fn delete(app: &Arc<AppState>, id: i64) -> Out {
    let inner = lock(app);
    let locomotive = load(&inner.db, id)?;
    if locomotives::in_use(&inner.db, id)? {
        return Err(ApiError::conflict("Locomotive is in use by a block or route"));
    }
    remove_picture_file(app, &locomotive);
    locomotives::delete(&inner.db, id)?;
    Ok(Reply::empty(204))
}

fn remove_picture_file(app: &Arc<AppState>, locomotive: &Locomotive) {
    if let Some(file) = &locomotive.image_file {
        let _ = fs::remove_file(app.images.join(file));
    }
}

fn upload_image(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let inner = lock(app);
    let mut locomotive = load(&inner.db, id)?;
    if call.body.is_empty() {
        return Err(ApiError::bad_request("No picture data was received"));
    }
    // The browser sends a form with a file part. The desktop app sends the picture's bytes as the body
    let (declared, data) = match multipart::file_part(call.content_type, call.body) {
        Some(part) => part,
        None => (call.content_type.to_string(), call.body.to_vec()),
    };
    let content_type = declared.split(';').next().unwrap_or_default().trim().to_ascii_lowercase();
    let Some((_, extension, signature)) = IMAGE_TYPES.iter().find(|(kind, _, _)| *kind == content_type) else {
        return Err(ApiError::bad_request(format!(
            "Picture must be a JPEG, PNG, or WebP image (received type {declared:?})"
        )));
    };
    if data.len() > IMAGE_MAX_BYTES {
        return Err(ApiError::new(413, "Picture must be 16 MB or smaller"));
    }
    let webp_ok = content_type != "image/webp" || data.get(8..12) == Some(b"WEBP");
    if !data.starts_with(signature) || !webp_ok {
        let head: String = data.iter().take(8).map(|byte| format!("{byte:02x}")).collect();
        return Err(ApiError::bad_request(format!(
            "The file is not a valid image of that type (received {} bytes starting {head}, type {declared:?})",
            data.len()
        )));
    }

    fs::create_dir_all(&app.images)
        .map_err(|error| ApiError::internal(format!("Could not create the picture folder: {error}")))?;
    remove_picture_file(app, &locomotive);
    let name = format!("{}.{}", locomotive.id, extension);
    fs::write(app.images.join(&name), &data)
        .map_err(|error| ApiError::internal(format!("Could not save the picture: {error}")))?;
    locomotive.image_file = Some(name);
    locomotives::save(&inner.db, &locomotive)?;
    Ok(Reply::json(200, json!(load(&inner.db, id)?)))
}

fn get_image(app: &Arc<AppState>, id: i64) -> Out {
    let inner = lock(app);
    let locomotive = load(&inner.db, id)?;
    let file = locomotive
        .image_file
        .filter(|name| app.images.join(name).is_file())
        .ok_or_else(|| ApiError::not_found_message("Locomotive has no picture"))?;
    let content_type = IMAGE_TYPES
        .iter()
        .find(|(_, extension, _)| file.ends_with(extension))
        .map_or("application/octet-stream", |(kind, _, _)| kind);
    let bytes = fs::read(app.images.join(&file))
        .map_err(|error| ApiError::internal(format!("Could not read the picture: {error}")))?;
    Ok(Reply::file(content_type, bytes))
}

fn delete_image(app: &Arc<AppState>, id: i64) -> Out {
    let inner = lock(app);
    let mut locomotive = load(&inner.db, id)?;
    remove_picture_file(app, &locomotive);
    locomotive.image_file = None;
    locomotives::save(&inner.db, &locomotive)?;
    Ok(Reply::json(200, json!(load(&inner.db, id)?)))
}
