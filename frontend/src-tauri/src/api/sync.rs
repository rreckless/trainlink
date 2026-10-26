//! Sync endpoints: what the last sync did, and sync now

use std::sync::Arc;

use serde_json::{json, Value};

use super::Call;
use crate::http::{Out, Reply};
use crate::state::AppState;
use crate::sync;

pub fn route(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", ["status"]) => Ok(Reply::json(200, status_json(app))),
        ("POST", ["now"]) => {
            let result = sync::sync_and_record(app);
            let mut body = status_json(app);
            if let Ok(report) = result {
                body["pulled"] = json!(report.pulled);
                body["pushed"] = json!(report.pushed);
                body["skipped"] = json!(report.skipped);
            }
            Ok(Reply::json(200, body))
        }
        _ => Err(crate::http::ApiError::new(405, "Method not allowed")),
    }
}

fn status_json(app: &Arc<AppState>) -> Value {
    let status = app.sync.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    json!({
        "signed_in": app.auth.session().is_some(),
        "last_synced": status.last_synced,
        "last_error": status.last_error,
        "skipped": status.skipped,
    })
}
