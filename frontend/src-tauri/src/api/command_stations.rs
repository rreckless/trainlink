//! Command station endpoints. Only the Z21 is supported so far. A station's port is the host and port it is reached at,
//! written as "host:port", and the host must be on this computer or the local network

use std::sync::Arc;

use serde_json::{json, Value};

use super::Call;
use crate::db::command_stations::{self, CommandStation};
use crate::db::is_unique_violation;
use crate::fields::{self, Body};
use crate::http::{ApiError, Out, Reply};
use crate::state::{AppState, Inner};
use crate::z21;

const NAME_MAX: usize = 40;
const SUPPORTED_TYPES: [&str; 1] = ["z21"];

fn lock(app: &Arc<AppState>) -> std::sync::MutexGuard<'_, Inner> {
    app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn route(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => list(app),
        ("POST", []) => create(app, call),
        ("GET", [id]) => get(app, super::id_segment(id)?),
        ("PATCH", [id]) => update(app, super::id_segment(id)?, call),
        ("DELETE", [id]) => delete(app, super::id_segment(id)?),
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

fn list(app: &Arc<AppState>) -> Out {
    let inner = lock(app);
    Ok(Reply::json(200, json!(command_stations::list(&inner.db)?)))
}

fn load(db: &rusqlite::Connection, id: i64) -> Result<CommandStation, ApiError> {
    command_stations::get(db, id)?.ok_or_else(|| ApiError::not_found("Command station", id))
}

fn get(app: &Arc<AppState>, id: i64) -> Out {
    let inner = lock(app);
    Ok(Reply::json(200, json!(load(&inner.db, id)?)))
}

fn check_name(name: &str) -> Result<String, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > NAME_MAX {
        return Err(ApiError::unprocessable(format!("Name must be 1 to {NAME_MAX} characters")));
    }
    Ok(trimmed.to_string())
}

fn check_type(station_type: &str) -> Result<String, ApiError> {
    if !SUPPORTED_TYPES.contains(&station_type) {
        return Err(ApiError::unprocessable(format!("Station type must be one of: {}", SUPPORTED_TYPES.join(", "))));
    }
    Ok(station_type.to_string())
}

/// "host:port", with a host on this computer or the local network and a port from 1 to 65535
fn check_port(port: &str) -> Result<String, ApiError> {
    let trimmed = port.trim();
    let (host, number) = trimmed
        .rsplit_once(':')
        .ok_or_else(|| ApiError::unprocessable("Port must be written as host:port, for example 10.0.0.111:21105"))?;
    if !z21::is_local_address(host) {
        return Err(ApiError::unprocessable("The station must be on this computer or your local network, given as an IP address"));
    }
    match number.parse::<u16>() {
        Ok(value) if value > 0 => Ok(trimmed.to_string()),
        _ => Err(ApiError::unprocessable("The port must be a number from 1 to 65535")),
    }
}

fn check_settings(value: &Value) -> Result<(), ApiError> {
    if !value.is_object() {
        return Err(ApiError::unprocessable("settings must be an object"));
    }
    Ok(())
}

fn create(app: &Arc<AppState>, call: &Call) -> Out {
    let body: Body = call.json_body()?;
    let name = check_name(&fields::required_str(&body, "name")?)?;
    let station_type = check_type(&fields::optional_str(&body, "station_type")?.unwrap_or_else(|| "z21".into()))?;
    let port = check_port(&fields::required_str(&body, "port")?)?;
    let settings = body.get("settings").cloned().unwrap_or_else(|| json!({}));
    check_settings(&settings)?;
    let inner = lock(app);
    let id = command_stations::insert(&inner.db, &name, &station_type, &port, &settings).map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("Command station name already exists")
        } else {
            error.into()
        }
    })?;
    command_stations::claim_pending(&inner.db, id, &name)?;
    Ok(Reply::json(201, json!(load(&inner.db, id)?)))
}

fn update(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body: Body = call.json_body()?;
    let inner = lock(app);
    let mut station = load(&inner.db, id)?;
    let old_name = station.name.clone();
    if let Some(name) = fields::optional_str(&body, "name")? {
        station.name = check_name(&name)?;
    }
    if let Some(station_type) = fields::optional_str(&body, "station_type")? {
        station.station_type = check_type(&station_type)?;
    }
    if let Some(port) = fields::optional_str(&body, "port")? {
        station.port = check_port(&port)?;
    }
    if let Some(settings) = body.get("settings") {
        check_settings(settings)?;
        station.settings = settings.clone();
    }
    command_stations::save(&inner.db, &station).map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("Command station name already exists")
        } else {
            error.into()
        }
    })?;
    if station.name != old_name {
        // Locomotives refer to their station by name, so each one is sent again with the new name
        command_stations::claim_pending(&inner.db, id, &station.name)?;
        inner.db.execute(
            "UPDATE locomotives SET dirty = 1, updated_at = ?2 WHERE command_station_id = ?1",
            rusqlite::params![id, crate::db::now()],
        )?;
    }
    Ok(Reply::json(200, json!(load(&inner.db, id)?)))
}

fn delete(app: &Arc<AppState>, id: i64) -> Out {
    let inner = lock(app);
    load(&inner.db, id)?;
    let using = command_stations::locomotives_using(&inner.db, id)?;
    if using > 0 {
        return Err(ApiError::conflict(format!("Command station is used by {using} locomotive{}", if using == 1 { "" } else { "s" })));
    }
    command_stations::delete(&inner.db, id)?;
    Ok(Reply::empty(204))
}
