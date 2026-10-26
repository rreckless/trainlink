//! Decoder profiles: the CVs and function names of a decoder type, which can be imported from a JSON file

use std::sync::Arc;

use serde_json::{json, Value};

use super::Call;
use crate::db::decoder_profiles::{self, Profile};
use crate::fields::{self, Body};
use crate::http::{ApiError, Out, Reply};
use crate::state::{AppState, Inner};

const NAME_MAX: usize = 60;
const CV_MAX_COUNT: usize = 256;
const FUNCTION_MAX_COUNT: usize = 32;

fn lock(app: &Arc<AppState>) -> std::sync::MutexGuard<'_, Inner> {
    app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn route(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => {
            let inner = lock(app);
            Ok(Reply::json(200, json!(decoder_profiles::list(&inner.db)?)))
        }
        ("POST", []) => create(app, call),
        ("GET", [id]) => {
            let inner = lock(app);
            Ok(Reply::json(200, json!(load(&inner.db, super::id_segment(id)?)?)))
        }
        ("DELETE", [id]) => delete(app, super::id_segment(id)?),
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

fn load(db: &rusqlite::Connection, id: i64) -> Result<Profile, ApiError> {
    decoder_profiles::get(db, id)?.ok_or_else(|| ApiError::not_found("Decoder profile", id))
}

fn text(value: Option<String>, label: &str, max: usize, required: bool) -> Result<String, ApiError> {
    let trimmed = value.unwrap_or_default().trim().to_string();
    if required && trimmed.is_empty() {
        return Err(ApiError::unprocessable(format!("{label} is required")));
    }
    if trimmed.chars().count() > max {
        return Err(ApiError::unprocessable(format!("{label} must be at most {max} characters")));
    }
    Ok(trimmed)
}

/// Each CV: a number from 1 to 1024, a name, a range inside 0 to 255, and whether it is read-only. No CV twice
fn check_cvs(value: &Value) -> Result<Value, ApiError> {
    let items = value.as_array().ok_or_else(|| ApiError::unprocessable("cvs must be a list"))?;
    if items.is_empty() || items.len() > CV_MAX_COUNT {
        return Err(ApiError::unprocessable(format!("A profile needs 1 to {CV_MAX_COUNT} CVs")));
    }
    let mut seen = Vec::new();
    let mut checked = Vec::new();
    for item in items {
        let cv = item.get("cv").and_then(Value::as_i64).filter(|cv| (1..=1024).contains(cv)).ok_or_else(|| ApiError::unprocessable("Each CV needs a number from 1 to 1024"))?;
        if seen.contains(&cv) {
            return Err(ApiError::unprocessable(format!("CV {cv} is listed twice")));
        }
        seen.push(cv);
        let name = text(item.get("name").and_then(Value::as_str).map(str::to_string), &format!("The name of CV {cv}"), NAME_MAX, true)?;
        let min = item.get("min").and_then(Value::as_i64).unwrap_or(0);
        let max = item.get("max").and_then(Value::as_i64).unwrap_or(255);
        if !(0..=255).contains(&min) || !(0..=255).contains(&max) || min > max {
            return Err(ApiError::unprocessable(format!("CV {cv} needs a range inside 0 to 255, with the minimum first")));
        }
        let read_only = item.get("read_only").and_then(Value::as_bool).unwrap_or(false);
        checked.push(json!({ "cv": cv, "name": name, "min": min, "max": max, "read_only": read_only }));
    }
    Ok(Value::Array(checked))
}

/// Function names: an index from 0 to 31 and a label. No index twice
fn check_functions(value: Option<&Value>) -> Result<Value, ApiError> {
    let Some(value) = value.filter(|value| !value.is_null()) else { return Ok(json!([])) };
    let items = value.as_array().ok_or_else(|| ApiError::unprocessable("functions must be a list"))?;
    if items.len() > FUNCTION_MAX_COUNT {
        return Err(ApiError::unprocessable(format!("A profile lists at most {FUNCTION_MAX_COUNT} functions")));
    }
    let mut seen = Vec::new();
    let mut checked = Vec::new();
    for item in items {
        let index = item.get("index").and_then(Value::as_i64).filter(|index| (0..=31).contains(index)).ok_or_else(|| ApiError::unprocessable("Each function needs an index from 0 to 31"))?;
        if seen.contains(&index) {
            return Err(ApiError::unprocessable(format!("Function {index} is listed twice")));
        }
        seen.push(index);
        let label = text(item.get("label").and_then(Value::as_str).map(str::to_string), &format!("The label of function {index}"), 24, true)?;
        checked.push(json!({ "index": index, "label": label }));
    }
    Ok(Value::Array(checked))
}

fn create(app: &Arc<AppState>, call: &Call) -> Out {
    let body: Body = call.json_body()?;
    let name = text(fields::optional_str(&body, "name")?, "Name", NAME_MAX, true)?;
    let manufacturer = text(fields::optional_str(&body, "manufacturer")?, "Manufacturer", NAME_MAX, false)?;
    let description = text(fields::optional_str(&body, "description")?, "Description", 240, false)?;
    let cvs = check_cvs(body.get("cvs").ok_or_else(|| ApiError::unprocessable("cvs is required"))?)?;
    let functions = check_functions(body.get("functions"))?;
    let inner = lock(app);
    let id = decoder_profiles::insert(&inner.db, &name, &manufacturer, &description, &cvs, &functions).map_err(|error| {
        if crate::db::is_unique_violation(&error) {
            ApiError::conflict("A decoder profile with this name already exists")
        } else {
            error.into()
        }
    })?;
    Ok(Reply::json(201, json!(load(&inner.db, id)?)))
}

fn delete(app: &Arc<AppState>, id: i64) -> Out {
    let inner = lock(app);
    let profile = load(&inner.db, id)?;
    if profile.builtin {
        return Err(ApiError::conflict("The built-in profile cannot be deleted"));
    }
    decoder_profiles::delete(&inner.db, id)?;
    Ok(Reply::empty(204))
}
