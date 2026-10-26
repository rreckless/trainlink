//! The REST API, served in-process. Paths, bodies, status codes, and error messages match the web backend,
//! so the frontend's calls work unchanged

pub mod auth;
pub mod consists;
pub mod layouts;
pub mod library;
pub mod locomotives;
pub mod multipart;
pub mod overview;
pub mod settings;
pub mod sync;
pub mod trackside;
pub mod z21;

use std::sync::Arc;

use serde_json::{json, Value};

use crate::fields::Body;
use crate::http::{ApiError, Out, Reply};
use crate::state::AppState;

/// One request, as the frontend sent it
pub struct Call<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a str,
    pub body: &'a [u8],
    pub content_type: &'a str,
}

impl Call<'_> {
    pub fn json_body(&self) -> Result<Body, ApiError> {
        crate::fields::object(self.body)
    }

    pub fn param(&self, name: &str) -> Option<String> {
        parse_query(self.query)
            .into_iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }
}

/// Answer one request
pub fn handle(app: &Arc<AppState>, call: &Call) -> Reply {
    if call.method == "OPTIONS" {
        return Reply::empty(204);
    }
    let segments: Vec<&str> = call.path.split('/').filter(|part| !part.is_empty()).collect();
    match route(app, call, &segments) {
        Ok(reply) => {
            if reply.status < 300 && changes_synced_data(call.method, call.path) {
                crate::sync::request_sync();
            }
            reply
        }
        Err(error) => error.into_reply(),
    }
}

/// Whether a request changed data that syncs: a write to layouts, the roster, or the track on a layout. Reads, sign-in,
/// the Z21, and sync itself do not. Moving time follows on the sync interval
pub fn changes_synced_data(method: &str, path: &str) -> bool {
    if method == "GET" {
        return false;
    }
    ["/api/layouts", "/api/locomotives", "/api/blocks", "/api/signals", "/api/feedbacks", "/api/routes", "/api/settings"]
        .iter()
        .any(|prefix| path == *prefix || path.strip_prefix(prefix).is_some_and(|rest| rest.starts_with('/')))
}

fn route(app: &Arc<AppState>, call: &Call, segments: &[&str]) -> Out {
    match segments {
        ["health"] => Ok(Reply::json(200, json!({ "status": "healthy", "version": "0.1.0", "database": "connected" }))),
        ["api", "auth", rest @ ..] => auth::route(app, call, rest),
        ["api", "sync", rest @ ..] => sync::route(app, call, rest),
        ["api", "settings", rest @ ..] => settings::route(app, call, rest),
        ["api", "layouts", rest @ ..] => layouts::route(app, call, rest),
        ["api", "locomotives", rest @ ..] => locomotives::route(app, call, rest),
        ["api", "consists", rest @ ..] => consists::route(app, call, rest),
        ["api", "overview", rest @ ..] => overview::route(app, call, rest),
        ["api", "blocks", rest @ ..] => trackside::blocks(app, call, rest),
        ["api", "signals", rest @ ..] => trackside::signals(app, call, rest),
        ["api", "feedbacks", rest @ ..] => trackside::feedbacks(app, call, rest),
        ["api", "routes", rest @ ..] => trackside::routes(app, call, rest),
        ["api", "z21", rest @ ..] => z21::route(app, call, rest),
        ["api", "track-library", rest @ ..] => library::route(call, rest),
        _ => Err(ApiError::new(404, "Not found")),
    }
}

/// An id in the path. A path that isn't a whole number is refused as FastAPI refuses it
pub fn id_segment(segment: &str) -> Result<i64, ApiError> {
    segment
        .parse::<i64>()
        .map_err(|_| ApiError::unprocessable(format!("Path id must be a whole number, not {segment:?}")))
}

/// Query string pairs, with percent-escapes and + decoded
pub fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => out.push(b' '),
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
                match hex.and_then(|digits| u8::from_str_radix(digits, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        index += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Comma-separated whole numbers from a query parameter, skipping empty entries
pub fn number_list(text: &str, message: &str) -> Result<Vec<i64>, ApiError> {
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<i64>().map_err(|_| ApiError::bad_request(message)))
        .collect()
}

/// The layout id query parameter, if given
pub fn optional_layout_id(call: &Call) -> Result<Option<i64>, ApiError> {
    match call.param("layout_id") {
        None => Ok(None),
        Some(text) => text
            .parse::<i64>()
            .map(Some)
            .map_err(|_| ApiError::unprocessable("layout_id must be a whole number")),
    }
}

/// True for a JSON number that is not a boolean
pub fn is_number(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Number(_)))
}
