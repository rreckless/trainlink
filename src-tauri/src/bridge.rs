//! Writes from the Android app's pages. The Android webview does not pass a request's body on to the app's own address,
//! so the page sends writes as a command, which carries the body. Reads still use the address.

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::api::{handle, Call};
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequest {
    pub method: String,
    /// The path and query, as the address would have them
    pub path: String,
    pub content_type: String,
    pub body_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiReply {
    pub status: u16,
    pub content_type: String,
    pub body_base64: String,
}

fn run(app: &AppHandle, request: &ApiRequest) -> Result<ApiReply, String> {
    let body = STANDARD.decode(&request.body_base64).map_err(|error| format!("The request body was not valid: {error}"))?;
    let (path, query) = request.path.split_once('?').unwrap_or((request.path.as_str(), ""));
    let state = app.state::<Arc<AppState>>();
    let reply = handle(
        state.inner(),
        &Call { method: &request.method, path, query, body: &body, content_type: &request.content_type },
    );
    Ok(ApiReply { status: reply.status, content_type: reply.content_type, body_base64: STANDARD.encode(reply.body) })
}

/// Answers one API request from the page. The work runs off the main thread, since sign-in and sync can take a while
#[tauri::command]
pub async fn api_call(app: AppHandle, request: ApiRequest) -> Result<ApiReply, String> {
    tauri::async_runtime::spawn_blocking(move || run(&app, &request)).await.map_err(|error| error.to_string())?
}
