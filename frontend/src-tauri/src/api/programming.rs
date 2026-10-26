//! Decoder programming: reading and writing CVs. On the programming track the Z21 answers, and the answer is read with
//! the status call. On the main track (POM) the Z21 gives no answer, so a POM write cannot be checked. Every write needs
//! confirm: true, and only one operation runs at a time

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::Call;
use crate::db::locomotives;
use crate::fields::{self, Body};
use crate::http::{ApiError, Out, Reply};
use crate::state::{AppState, Inner};
use crate::z21::commands;

/// How long the Z21 has to answer a programming operation on the programming track
const ANSWER_WAIT: Duration = Duration::from_secs(30);

fn lock(app: &Arc<AppState>) -> std::sync::MutexGuard<'_, Inner> {
    app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn route(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", ["status"]) => status(app),
        ("POST", ["read"]) => read(app, call),
        ("POST", ["write"]) => write(app, call),
        ("POST", ["pom", "write"]) => pom_write(app, call),
        ("POST", ["pom", "bit"]) => pom_bit(app, call),
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

fn check_cv(cv: i64) -> Result<u16, ApiError> {
    if !(1..=1024).contains(&cv) {
        return Err(ApiError::unprocessable("CV must be 1 to 1024"));
    }
    Ok(cv as u16)
}

fn check_value(value: i64) -> Result<u8, ApiError> {
    if !(0..=255).contains(&value) {
        return Err(ApiError::unprocessable("Value must be 0 to 255"));
    }
    Ok(value as u8)
}

fn require_confirmation(body: &Body) -> Result<(), ApiError> {
    if fields::optional_bool(body, "confirm")? != Some(true) {
        return Err(ApiError::unprocessable("Confirm the write first: a wrong CV value can stop a decoder working"));
    }
    Ok(())
}

/// Send a frame, and refuse if no operation is already waiting for an answer
fn send_now(inner: &mut Inner, frame: &[u8]) -> Result<(), ApiError> {
    if inner.z21.programming.pending.is_some() {
        return Err(ApiError::conflict("A programming operation is already running"));
    }
    if !inner.z21.send(frame) {
        return Err(ApiError::internal("Could not send the programming command to the Z21"));
    }
    Ok(())
}

fn read(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let cv = check_cv(fields::required_int(&body, "cv")?)?;
    let frame = commands::cv_read(cv).map_err(ApiError::unprocessable)?;
    let mut inner = lock(app);
    super::z21::ready_check(&inner, "Programming")?;
    send_now(&mut inner, &frame)?;
    inner.z21.programming.start(cv, "read", None, Instant::now());
    Ok(Reply::json(202, json!({ "status": "reading", "cv": cv })))
}

fn write(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let cv = check_cv(fields::required_int(&body, "cv")?)?;
    let value = check_value(fields::required_int(&body, "value")?)?;
    require_confirmation(&body)?;
    let frame = commands::cv_write(cv, value).map_err(ApiError::unprocessable)?;
    let mut inner = lock(app);
    super::z21::ready_check(&inner, "Programming")?;
    send_now(&mut inner, &frame)?;
    inner.z21.programming.start(cv, "write", Some(value), Instant::now());
    Ok(Reply::json(202, json!({ "status": "writing", "cv": cv, "value": value })))
}

/// The locomotive a POM command goes to, which must be on the roster
fn locomotive_address(inner: &Inner, body: &Body) -> Result<i64, ApiError> {
    let id = fields::required_int(body, "locomotive_id")?;
    let loco = locomotives::get(&inner.db, id)?.ok_or_else(|| ApiError::not_found("Locomotive", id))?;
    Ok(loco.dcc_address)
}

fn pom_write(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let cv = check_cv(fields::required_int(&body, "cv")?)?;
    let value = check_value(fields::required_int(&body, "value")?)?;
    require_confirmation(&body)?;
    let mut inner = lock(app);
    let address = locomotive_address(&inner, &body)?;
    super::z21::ready_check(&inner, "Programming")?;
    let frame = commands::pom_write_byte(address, cv, value).map_err(ApiError::unprocessable)?;
    if !inner.z21.send(&frame) {
        return Err(ApiError::internal("Could not send the programming command to the Z21"));
    }
    // The Z21 gives no answer on the main track, so the write is recorded as sent, not checked
    inner.z21.programming.last = Some(crate::z21::Outcome { cv, kind: "pom_write", value: Some(value), error: None });
    Ok(Reply::json(200, json!({ "status": "sent", "cv": cv, "value": value, "checked": false })))
}

fn pom_bit(app: &Arc<AppState>, call: &Call) -> Out {
    let body = call.json_body()?;
    let cv = check_cv(fields::required_int(&body, "cv")?)?;
    let bit = fields::required_int(&body, "bit")?;
    if !(0..=7).contains(&bit) {
        return Err(ApiError::unprocessable("Bit must be 0 to 7"));
    }
    let on = fields::required_bool(&body, "on")?;
    require_confirmation(&body)?;
    let mut inner = lock(app);
    let address = locomotive_address(&inner, &body)?;
    super::z21::ready_check(&inner, "Programming")?;
    let frame = commands::pom_write_bit(address, cv, bit as u8, on).map_err(ApiError::unprocessable)?;
    if !inner.z21.send(&frame) {
        return Err(ApiError::internal("Could not send the programming command to the Z21"));
    }
    inner.z21.programming.last = Some(crate::z21::Outcome { cv, kind: "pom_bit", value: None, error: None });
    Ok(Reply::json(200, json!({ "status": "sent", "cv": cv, "bit": bit, "on": on, "checked": false })))
}

/// What is running, and what finished last. A programming operation with no answer in time is reported as failed
fn status(app: &Arc<AppState>) -> Out {
    let mut inner = lock(app);
    let expired = inner.z21.programming.pending.as_ref().is_some_and(|pending| pending.started.elapsed() > ANSWER_WAIT);
    if expired {
        inner.z21.programming.finish_with_error("No answer from the Z21 in time");
    }
    let pending = inner.z21.programming.pending.as_ref().map(|pending| json!({ "cv": pending.cv, "kind": pending.kind, "value": pending.value }));
    let last = inner.z21.programming.last.as_ref().map(outcome_json);
    Ok(Reply::json(200, json!({ "pending": pending, "last": last })))
}

fn outcome_json(outcome: &crate::z21::Outcome) -> Value {
    json!({ "cv": outcome.cv, "kind": outcome.kind, "value": outcome.value, "error": outcome.error })
}
