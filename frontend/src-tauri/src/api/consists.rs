//! Consist endpoints: the groups, and driving a whole group through the Z21. A drive command is sent to each member
//! in turn, with the direction flipped for a member that runs reversed. The consist address is stored here; the
//! decoders follow it once their CV19 is set, which the app does not do yet

use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};

use super::Call;
use crate::drivers::Driver;
use crate::db::consists::{self, Consist, MemberInput, ADDRESS_MAX, ADDRESS_MIN};
use crate::db::is_unique_violation;
use crate::db::locomotives;
use crate::fields::{self, Body};
use crate::http::{ApiError, Out, Reply};
use crate::state::{AppState, Inner};
use crate::z21::commands;

const NAME_MAX: usize = 40;
/// A consist is a group, so it needs at least two locomotives
const MEMBERS_MIN: usize = 2;

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
        ("POST", [id, "speed"]) => set_speed(app, super::id_segment(id)?, call),
        ("POST", [id, "function"]) => set_function(app, super::id_segment(id)?, call),
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

fn list(app: &Arc<AppState>) -> Out {
    let inner = lock(app);
    Ok(Reply::json(200, json!(consists::list(&inner.db)?)))
}

fn get(app: &Arc<AppState>, id: i64) -> Out {
    let inner = lock(app);
    Ok(Reply::json(200, json!(load(&inner.db, id)?)))
}

fn load(db: &rusqlite::Connection, id: i64) -> Result<Consist, ApiError> {
    consists::get(db, id)?.ok_or_else(|| ApiError::not_found("Consist", id))
}

/// The consist's name, trimmed. It must not be blank
fn check_name(name: &str) -> Result<String, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > NAME_MAX {
        return Err(ApiError::unprocessable(format!("Name must be 1 to {NAME_MAX} characters")));
    }
    Ok(trimmed.to_string())
}

/// The consist address must be a DCC short address
fn check_address(address: i64) -> Result<i64, ApiError> {
    if !(ADDRESS_MIN..=ADDRESS_MAX).contains(&address) {
        return Err(ApiError::unprocessable(format!("Consist address must be {ADDRESS_MIN} to {ADDRESS_MAX}")));
    }
    Ok(address)
}

/// The members in order: each is { "locomotive_id": n, "reversed": bool }. The first is the lead
fn parse_members(body: &Value) -> Result<Vec<MemberInput>, ApiError> {
    let items = body
        .as_array()
        .ok_or_else(|| ApiError::unprocessable("members must be a list"))?;
    items
        .iter()
        .map(|item| {
            let id = item
                .get("locomotive_id")
                .and_then(Value::as_i64)
                .ok_or_else(|| ApiError::unprocessable("Each member needs a locomotive_id"))?;
            let reversed = item.get("reversed").and_then(Value::as_bool).unwrap_or(false);
            Ok((id, reversed))
        })
        .collect()
}

/// Members must exist, appear once, be unique to this consist, and not already be in another consist
fn check_members(db: &rusqlite::Connection, consist_id: Option<i64>, members: &[MemberInput]) -> Result<(), ApiError> {
    if members.len() < MEMBERS_MIN {
        return Err(ApiError::unprocessable(format!("A consist needs at least {MEMBERS_MIN} locomotives")));
    }
    for (index, (id, _)) in members.iter().enumerate() {
        if members[..index].iter().any(|(earlier, _)| earlier == id) {
            return Err(ApiError::unprocessable("A locomotive can appear only once in a consist"));
        }
        if locomotives::get(db, *id)?.is_none() {
            return Err(ApiError::not_found("Locomotive", *id));
        }
        match consists::consist_of(db, *id)? {
            Some(owner) if Some(owner) != consist_id => {
                return Err(ApiError::conflict("A locomotive is already in another consist"));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Refuse an address another consist already uses
fn check_address_free(db: &rusqlite::Connection, address: i64, consist_id: Option<i64>) -> Result<(), ApiError> {
    if consists::address_taken(db, address, consist_id)? {
        return Err(ApiError::conflict("Consist address is already used by another consist"));
    }
    Ok(())
}

fn create(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let name = check_name(&fields::required_str(&body, "name")?)?;
    let members = parse_members(body.get("members").unwrap_or(&Value::Null))?;
    let mut inner = lock(app);
    let address = match fields::optional_int(&body, "address")? {
        Some(address) => check_address(address)?,
        None => consists::free_address(&inner.db)?
            .ok_or_else(|| ApiError::conflict("Every consist address is already in use"))?,
    };
    check_address_free(&inner.db, address, None)?;
    check_members(&inner.db, None, &members)?;
    let id = consists::insert(&mut inner.db, &name, address, &members).map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("Consist name already exists")
        } else {
            error.into()
        }
    })?;
    Ok(Reply::json(201, json!(load(&inner.db, id)?)))
}

fn update(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body: Body = call.json_body()?;
    let name = match fields::optional_str(&body, "name")? {
        Some(name) => Some(check_name(&name)?),
        None => None,
    };
    let address = match fields::optional_int(&body, "address")? {
        Some(address) => Some(check_address(address)?),
        None => None,
    };
    let members = match body.get("members") {
        Some(value) => Some(parse_members(value)?),
        None => None,
    };
    let mut inner = lock(app);
    load(&inner.db, id)?;
    if let Some(address) = address {
        check_address_free(&inner.db, address, Some(id))?;
    }
    if let Some(members) = &members {
        check_members(&inner.db, Some(id), members)?;
    }
    let updated = consists::update(&mut inner.db, id, name.as_deref(), address, members.as_deref()).map_err(|error| {
        if is_unique_violation(&error) {
            ApiError::conflict("Consist name already exists")
        } else {
            error.into()
        }
    })?;
    if !updated {
        return Err(ApiError::not_found("Consist", id));
    }
    Ok(Reply::json(200, json!(load(&inner.db, id)?)))
}

fn delete(app: &Arc<AppState>, id: i64) -> Out {
    let inner = lock(app);
    if !consists::delete(&inner.db, id)? {
        return Err(ApiError::not_found("Consist", id));
    }
    Ok(Reply::empty(204))
}

/// Send one speed and direction to every member. A reversed member runs the other way
fn set_speed(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    let speed = fields::required_int(&body, "speed")?;
    let forward = fields::optional_bool(&body, "direction")?.unwrap_or(true);
    if !(0..=126).contains(&speed) {
        return Err(ApiError::bad_request("Speed must be 0-126"));
    }
    let mut inner = lock(app);
    super::z21::ready_check(&inner, "Consist")?;
    let consist = load(&inner.db, id)?;
    // Every member is checked before any locomotive moves, so one bad address leaves the whole consist where it was
    for member in &consist.members {
        commands::loco_drive(member.dcc_address, speed, forward != member.reversed).map_err(ApiError::bad_request)?;
    }
    for member in &consist.members {
        Driver::drive(&mut inner.z21, member.dcc_address, speed, forward != member.reversed)
            .map_err(|error| super::z21::driver_error(error, "Failed to set consist speed"))?;
    }
    let inner = &mut *inner;
    let Inner { db, moving, .. } = inner;
    for member in &consist.members {
        moving.observe(db, member.dcc_address, Some(speed), Instant::now());
    }
    let direction = if forward { "forward" } else { "reverse" };
    Ok(Reply::json(200, json!({ "status": "speed_set", "consist_id": id, "speed": speed, "direction": direction })))
}

/// Switch one function on or off for every member of the consist
fn set_function(app: &Arc<AppState>, id: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    let index = fields::required_int(&body, "function")?;
    let on = fields::required_bool(&body, "on")?;
    if !(0..=commands::MAX_FUNCTION as i64).contains(&index) {
        return Err(ApiError::unprocessable(format!("function must be 0 to {}", commands::MAX_FUNCTION)));
    }
    let mut inner = lock(app);
    super::z21::ready_check(&inner, "Consist")?;
    let consist = load(&inner.db, id)?;
    for member in &consist.members {
        commands::set_function(member.dcc_address, index as u8, on).map_err(ApiError::bad_request)?;
    }
    for member in &consist.members {
        Driver::set_function(&mut inner.z21, member.dcc_address, index as u8, on)
            .map_err(|error| super::z21::driver_error(error, "Failed to set consist function"))?;
    }
    Ok(Reply::json(200, json!({ "status": "function_set", "consist_id": id, "function": index, "on": on })))
}
