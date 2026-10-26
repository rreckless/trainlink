//! Contract tests for the API, run against an in-memory database. None of them opens a socket to the Z21

mod consists;
mod overview;
mod layouts;
mod locomotives;
mod trackside;
mod z21;
mod z21_link;
mod moving;
mod anyrail;
mod auth;
mod server;
mod sync;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::Value;

use crate::api::{handle, Call};
use crate::state::AppState;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh application state with an empty database and its own picture folder
pub fn app() -> Arc<AppState> {
    let folder: PathBuf = std::env::temp_dir().join(format!(
        "trainlink-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let db = crate::db::open_in_memory().expect("in-memory database");
    Arc::new(AppState::new(db, folder))
}

/// Make one call. Returns the status and the JSON body (Null when there is none)
pub fn send(app: &Arc<AppState>, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let bytes = body.map(|value| value.to_string().into_bytes()).unwrap_or_default();
    let reply = handle(app, &Call { method, path, query, body: &bytes, content_type: "application/json" });
    let value = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
    (reply.status, value)
}

/// The detail message of an error reply
pub fn detail(value: &Value) -> &str {
    value["detail"].as_str().unwrap_or_default()
}

/// A layout with a fixed name, for the tests that need one
pub fn layout(app: &Arc<AppState>, name: &str) -> i64 {
    let body = serde_json::json!({ "name": name, "width": 800, "height": 600, "scale": "N" });
    let (status, value) = send(app, "POST", "/api/layouts", Some(body));
    assert_eq!(status, 201, "{value}");
    value["id"].as_i64().unwrap()
}
