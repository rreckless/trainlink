//! Reading request bodies. Missing, null, and wrongly typed fields are refused the way the REST API did

use serde_json::{Map, Value};

use crate::http::ApiError;

pub type Body = Map<String, Value>;

/// The JSON object a request body must be
pub fn object(bytes: &[u8]) -> Result<Body, ApiError> {
    if bytes.is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(ApiError::unprocessable("The body must be a JSON object")),
        Err(error) => Err(ApiError::unprocessable(format!("Invalid JSON: {error}"))),
    }
}

fn wrong(key: &str, kind: &str) -> ApiError {
    ApiError::unprocessable(format!("{key} must be {kind}"))
}

pub fn required_str(body: &Body, key: &str) -> Result<String, ApiError> {
    match body.get(key) {
        Some(Value::String(text)) => Ok(text.clone()),
        Some(_) => Err(wrong(key, "a string")),
        None => Err(ApiError::unprocessable(format!("{key} is required"))),
    }
}

pub fn required_float(body: &Body, key: &str) -> Result<f64, ApiError> {
    match body.get(key) {
        Some(value) => number(value).ok_or_else(|| wrong(key, "a number")),
        None => Err(ApiError::unprocessable(format!("{key} is required"))),
    }
}

pub fn required_int(body: &Body, key: &str) -> Result<i64, ApiError> {
    match body.get(key) {
        Some(value) => integer(value).ok_or_else(|| wrong(key, "an integer")),
        None => Err(ApiError::unprocessable(format!("{key} is required"))),
    }
}

/// An optional field. Absent or null gives None
pub fn optional_str(body: &Body, key: &str) -> Result<Option<String>, ApiError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(wrong(key, "a string")),
    }
}

pub fn optional_float(body: &Body, key: &str) -> Result<Option<f64>, ApiError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => number(value).map(Some).ok_or_else(|| wrong(key, "a number")),
    }
}

pub fn optional_int(body: &Body, key: &str) -> Result<Option<i64>, ApiError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => integer(value).map(Some).ok_or_else(|| wrong(key, "an integer")),
    }
}

pub fn optional_bool(body: &Body, key: &str) -> Result<Option<bool>, ApiError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(flag)) => Ok(Some(*flag)),
        Some(_) => Err(wrong(key, "true or false")),
    }
}

/// Explicit nulls in a PATCH body are refused, since none of these fields may be cleared that way
pub fn reject_nulls(changes: &Body) -> Result<(), ApiError> {
    let nulls: Vec<&str> = changes
        .iter()
        .filter(|(_, value)| value.is_null())
        .map(|(key, _)| key.as_str())
        .collect();
    if nulls.is_empty() {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!("Fields cannot be null: {}", nulls.join(", "))))
    }
}

/// The fields of a PATCH body that are in `allowed`, in the body's order. Unknown fields are ignored
pub fn changes(body: &Body, allowed: &[&str]) -> Body {
    body.iter()
        .filter(|(key, _)| allowed.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        _ => None,
    }
}

fn integer(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64().filter(|_| !number.is_f64()),
        _ => None,
    }
}

pub fn required_bool(body: &Body, key: &str) -> Result<bool, ApiError> {
    match body.get(key) {
        Some(Value::Bool(flag)) => Ok(*flag),
        Some(_) => Err(wrong(key, "true or false")),
        None => Err(ApiError::unprocessable(format!("{key} is required"))),
    }
}
