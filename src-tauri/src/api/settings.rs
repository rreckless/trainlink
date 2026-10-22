//! The account's settings. Each change is kept here and sent to the cloud, so every device uses the same settings

use std::sync::Arc;

use serde_json::{json, Value};

use super::Call;
use crate::db::settings::{self, NAMES};
use crate::http::{ApiError, Out, Reply};
use crate::state::AppState;

pub fn route(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => list(app),
        ("PUT", [name]) => set(app, call, name),
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

fn list(app: &Arc<AppState>) -> Out {
    let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    Ok(Reply::json(200, Value::Object(settings::all(&inner.db)?)))
}

fn set(app: &Arc<AppState>, call: &Call, name: &str) -> Out {
    let known = NAMES.iter().copied().find(|known| *known == name).ok_or_else(|| ApiError::new(404, "Unknown setting"))?;
    let body = call.json_body()?;
    let value = body.get("value").cloned().unwrap_or(Value::Null);
    check(known, &value)?;
    let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    settings::set(&inner.db, known, &value)?;
    Ok(Reply::json(200, json!({ "name": known, "value": value })))
}

/// The value a setting may hold
fn check(name: &str, value: &Value) -> Result<(), ApiError> {
    match name {
        "z21.host" => {
            let host = value.as_str().ok_or_else(|| ApiError::unprocessable("z21.host must be a string"))?;
            if !crate::z21::is_local_address(host) {
                return Err(ApiError::unprocessable("The Z21 must be on this computer or your local network, given as an IP address"));
            }
        }
        "z21.port" => {
            value
                .as_i64()
                .filter(|port| (1..=65535).contains(port))
                .ok_or_else(|| ApiError::unprocessable("z21.port must be a port number from 1 to 65535"))?;
        }
        _ => {}
    }
    Ok(())
}
