//! Z21 endpoints: connection, track state, turnouts, locomotives, and emergency stop

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::{number_list, Call};
use crate::db::{command_stations, layouts};
use crate::drivers::{Driver, DriverError};
use crate::http::{ApiError, Out, Reply};
use crate::state::{AppState, Inner};
use crate::z21::{self, commands, frames::MAX_ADDRESS, Z21};

const MIN_TURNOUT: i64 = 1;
const MAX_TURNOUT: i64 = 2048;
/// How long a turnout output stays active before it is switched off, as the spec's example does
/// How long a turnout read waits for the Z21's answers. The Z21 answers on the LAN within milliseconds
const TURNOUT_REPLY_WAIT: Duration = Duration::from_millis(150);

fn lock(app: &Arc<AppState>) -> std::sync::MutexGuard<'_, Inner> {
    app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn route(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("POST", ["connect"]) => connect(app, call),
        ("POST", ["disconnect"]) => {
            Driver::disconnect(&mut lock(app).z21);
            Ok(Reply::json(200, json!({ "status": "disconnected" })))
        }
        ("GET", ["status"]) => status(app),
        ("GET", ["feedback"]) => feedback(app),
        ("POST", ["turnouts", turnout, "throw"]) => throw(app, super::id_segment(turnout)?, call),
        ("GET", ["turnouts"]) => turnouts(app, call),
        ("POST", ["loco", "function"]) => set_function(app, call),
        ("GET", ["locos"]) => locos(app, call),
        ("POST", ["track-power", "on"]) => track_power_on(app),
        ("POST", ["track-power", "off"]) => track_power_off(app),
        ("POST", ["loco", "speed"]) => set_speed(app, call),
        ("POST", ["loco", "info"]) => loco_info(app, call),
        ("POST", ["emergency-stop"]) => emergency_stop(app, call),
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

fn connect(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let host = crate::fields::optional_str(&body, "host")?.unwrap_or_else(|| z21::DEFAULT_HOST.into());
    if !z21::is_local_address(&host) {
        return Err(ApiError::unprocessable("The Z21 must be on this computer or your local network, given as an IP address"));
    }
    let port = crate::fields::optional_int(&body, "port")?.unwrap_or(z21::DEFAULT_PORT as i64);
    let port = u16::try_from(port).map_err(|_| ApiError::unprocessable("port must be 0 to 65535"))?;
    if !<Z21 as Driver>::connect(app, &host, port) {
        return Err(ApiError::internal(format!("Failed to connect to Z21 at {host}:{port}")));
    }
    Ok(Reply::json(200, json!({ "status": "connected", "host": host, "port": port })))
}

fn status(app: &Arc<AppState>) -> Out {
    let inner = lock(app);
    // The answer comes back as an update, so a failure to ask is not an error for the status
    let _ = inner.z21.request_status();
    Ok(Reply::json(200, inner.z21.status_json()))
}

fn feedback(app: &Arc<AppState>) -> Out {
    let inner = lock(app);
    inner.z21.request_feedback();
    let inputs: Vec<Value> = inner
        .z21
        .feedback_inputs()
        .iter()
        .map(|input| json!({ "module": input.module, "input": input.input, "occupied": input.occupied }))
        .collect();
    Ok(Reply::json(200, json!({ "connected": inner.z21.connected, "inputs": inputs })))
}

/// The placed piece of the turnout with this number, on the first layout that has it
fn placed_turnout(inner: &Inner, turnout: i64) -> Result<Option<Value>, ApiError> {
    for layout in layouts::list(&inner.db)? {
        let pieces = layout.elements.get("track_pieces").and_then(Value::as_array).cloned().unwrap_or_default();
        if let Some(placed) = pieces.into_iter().find(|placed| placed.get("dcc_address").and_then(Value::as_i64) == Some(turnout)) {
            return Ok(Some(placed));
        }
    }
    Ok(None)
}

/// The name of the command station this device has at the address the Z21 is connected to, if there is one
fn connected_station(inner: &Inner) -> Result<Option<String>, ApiError> {
    let address = format!("{}:{}", inner.z21.host, inner.z21.port);
    Ok(command_stations::list(&inner.db)?.into_iter().find(|station| station.port == address).map(|station| station.name))
}

/// Refuse a throw for a turnout whose command station is not the one connected
fn check_station(inner: &Inner, turnout: i64, placed: Option<&Value>) -> Result<(), ApiError> {
    let Some(station) = placed.and_then(|placed| placed.get("command_station")).and_then(Value::as_str) else { return Ok(()) };
    if connected_station(inner)?.as_deref() != Some(station) {
        return Err(ApiError::conflict(format!("Turnout {turnout} is on command station {station}, which is not the one connected")));
    }
    Ok(())
}

/// The answer for a command the driver did not send. `failure` says what went wrong when the command could not be sent
pub(super) fn driver_error(error: DriverError, failure: &str) -> ApiError {
    match error {
        DriverError::Invalid(message) => ApiError::bad_request(message),
        DriverError::NotConnected => ApiError::new(503, "Z21 not connected"),
        DriverError::Failed => ApiError::internal(failure),
    }
}

pub(super) fn ready_check(inner: &Inner, what: &str) -> Result<(), ApiError> {
    if !inner.z21.connected {
        return Err(ApiError::new(503, "Z21 not connected"));
    }
    let state = inner.z21.track_state();
    if state != "ready" {
        return Err(ApiError::new(409, format!("Track is not ready ({state}). {what} commands are blocked")));
    }
    Ok(())
}

fn throw(app: &Arc<AppState>, turnout: i64, call: &Call) -> Out {
    let body = call.json_body()?;
    let output = crate::fields::required_int(&body, "output")?;
    if !(0..=1).contains(&output) {
        return Err(ApiError::unprocessable("output must be 0 or 1"));
    }
    let switching = {
        let mut inner = lock(app);
        if !inner.z21.connected {
            return Err(ApiError::new(503, "Z21 not connected"));
        }
        let state = inner.z21.track_state();
        if state != "ready" {
            return Err(ApiError::conflict(format!("Track is not ready ({state}). Turnout commands are blocked")));
        }
        if !(MIN_TURNOUT..=MAX_TURNOUT).contains(&turnout) {
            return Err(ApiError::bad_request(format!("Turnout must be {MIN_TURNOUT}-{MAX_TURNOUT}")));
        }
        let placed = placed_turnout(&inner, turnout)?;
        check_station(&inner, turnout, placed.as_ref())?;
        Driver::throw_turnout(&mut inner.z21, turnout, output as u8, true).map_err(|error| driver_error(error, "Failed to throw the turnout"))?;
        placed.map(|placed| layouts::turnout_settings(&placed)).unwrap_or_else(|| layouts::turnout_settings(&json!({})))
    };
    std::thread::sleep(Duration::from_millis(switching["throw_ms"].as_u64().unwrap_or(layouts::DEFAULT_THROW_MS as u64)));
    let mut inner = lock(app);
    Driver::throw_turnout(&mut inner.z21, turnout, output as u8, false).map_err(|error| driver_error(error, "Failed to throw the turnout"))?;
    // With position feedback on, ask the Z21 where the turnout is now. The answer updates the turnouts list
    if switching["feedback"].as_bool().unwrap_or(false) {
        let _ = inner.z21.request_turnout_info(turnout);
    }
    Ok(Reply::json(200, json!({ "status": "throw_sent", "turnout": turnout, "output": output })))
}

fn turnouts(app: &Arc<AppState>, call: &Call) -> Out {
    let text = call.param("addresses").unwrap_or_default();
    let numbers = number_list(&text, "Turnouts must be numbers")?;
    if numbers.iter().any(|turnout| !(MIN_TURNOUT..=MAX_TURNOUT).contains(turnout)) {
        return Err(ApiError::bad_request(format!("Turnouts must be {MIN_TURNOUT}-{MAX_TURNOUT}")));
    }
    let asked = {
        let inner = lock(app);
        let connected = inner.z21.connected();
        if connected {
            for turnout in &numbers {
                let _ = inner.z21.request_turnout_info(*turnout);
            }
        }
        connected
    };
    // The answers are applied when they arrive, so wait for them. Otherwise this reply would show the old state, and the
    // page would show it until the next read
    if asked {
        std::thread::sleep(TURNOUT_REPLY_WAIT);
    }
    let inner = lock(app);
    let states: serde_json::Map<String, Value> = numbers
        .iter()
        .map(|turnout| {
            let output = inner.z21.turnouts.get(turnout).copied().flatten();
            (turnout.to_string(), json!(output))
        })
        .collect();
    Ok(Reply::json(200, json!({ "connected": inner.z21.connected, "states": states })))
}

fn set_function(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let address = crate::fields::required_int(&body, "address")?;
    let index = crate::fields::required_int(&body, "function")?;
    let on = crate::fields::required_bool(&body, "on")?;
    if !(1..=9999).contains(&address) {
        return Err(ApiError::unprocessable("address must be 1 to 9999"));
    }
    if !(0..=commands::MAX_FUNCTION as i64).contains(&index) {
        return Err(ApiError::unprocessable(format!("function must be 0 to {}", commands::MAX_FUNCTION)));
    }
    let mut inner = lock(app);
    ready_check(&inner, "Locomotive")?;
    Driver::set_function(&mut inner.z21, address, index as u8, on).map_err(|error| driver_error(error, "Failed to set the function"))?;
    Ok(Reply::json(200, json!({ "status": "function_set", "address": address, "function": index, "on": on })))
}

fn locos(app: &Arc<AppState>, call: &Call) -> Out {
    let text = call.param("addresses").unwrap_or_default();
    let addresses = number_list(&text, "Addresses must be numbers")?;
    if addresses.iter().any(|address| !(1..=MAX_ADDRESS).contains(address)) {
        return Err(ApiError::bad_request("Addresses must be 1-9999"));
    }
    let mut inner = lock(app);
    if inner.z21.connected {
        for address in &addresses {
            let _ = inner.z21.request_loco_info(*address);
        }
    }
    let inner = &mut *inner;
    let Inner { db, z21, moving, .. } = inner;
    moving.flush_if_due(db, std::time::Instant::now());
    let states: serde_json::Map<String, Value> = addresses
        .iter()
        .map(|address| {
            let state = z21.locos.get(address).map(|loco| {
                json!({ "speed": loco.speed, "direction": loco.direction, "functions": loco.functions })
            });
            (address.to_string(), state.unwrap_or(Value::Null))
        })
        .collect();
    Ok(Reply::json(200, json!({ "connected": z21.connected, "states": states })))
}

fn track_power_on(app: &Arc<AppState>) -> Out {
    let mut inner = lock(app);
    if !inner.z21.connected {
        return Err(ApiError::new(503, "Z21 not connected"));
    }
    if inner.z21.track_state() == "short_circuit" {
        return Err(ApiError::conflict("Short circuit still reported. Fix the short, then resume track power"));
    }
    Driver::track_power(&mut inner.z21, true).map_err(|error| driver_error(error, "Could not send the track power command to the Z21"))?;
    Ok(Reply::json(200, json!({ "status": "track_power_on_sent" })))
}

/// Cut the track power. Allowed in any state, so it is never refused for a fault: it is the way to make the track safe
fn track_power_off(app: &Arc<AppState>) -> Out {
    let mut inner = lock(app);
    if !inner.z21.connected {
        return Err(ApiError::new(503, "Z21 not connected"));
    }
    Driver::track_power(&mut inner.z21, false).map_err(|error| driver_error(error, "Could not send the track power command to the Z21"))?;
    Ok(Reply::json(200, json!({ "status": "track_power_off_sent" })))
}

fn set_speed(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let address = crate::fields::required_int(&body, "address")?;
    let speed = crate::fields::required_int(&body, "speed")?;
    let forward = crate::fields::optional_bool(&body, "direction")?.unwrap_or(true);
    let mut inner = lock(app);
    ready_check(&inner, "Locomotive")?;
    if !(0..=126).contains(&speed) {
        return Err(ApiError::bad_request("Speed must be 0-126"));
    }
    if !(1..=MAX_ADDRESS).contains(&address) {
        return Err(ApiError::bad_request("Address must be 1-9999"));
    }
    Driver::drive(&mut inner.z21, address, speed, forward).map_err(|error| driver_error(error, "Failed to set locomotive speed"))?;
    let inner = &mut *inner;
    let Inner { db, moving, .. } = inner;
    moving.observe(db, address, Some(speed), std::time::Instant::now());
    let direction = if forward { "forward" } else { "reverse" };
    Ok(Reply::json(200, json!({ "status": "speed_set", "address": address, "speed": speed, "direction": direction })))
}

fn loco_info(app: &Arc<AppState>, call: &Call) -> Out {
    let address = call
        .param("address")
        .and_then(|text| text.parse::<i64>().ok())
        .ok_or_else(|| ApiError::unprocessable("address must be a whole number"))?;
    let inner = lock(app);
    if !inner.z21.connected {
        return Err(ApiError::new(503, "Z21 not connected"));
    }
    if !(1..=MAX_ADDRESS).contains(&address) {
        return Err(ApiError::bad_request("Address must be 1-9999"));
    }
    inner.z21.request_loco_info(address).map_err(|error| driver_error(error, "Failed to get locomotive info"))?;
    Ok(Reply::json(200, json!({ "address": address, "status": "requested" })))
}

fn emergency_stop(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let confirm = crate::fields::optional_bool(&body, "confirm")?.unwrap_or(true);
    if !confirm {
        return Err(ApiError::bad_request("Emergency stop not confirmed"));
    }
    let mut inner = lock(app);
    Driver::emergency_stop(&mut inner.z21).map_err(|error| driver_error(error, "Failed to send emergency stop"))?;
    Ok(Reply::json(200, json!({ "status": "emergency_stop_sent", "message": "All locomotives stopped" })))
}
