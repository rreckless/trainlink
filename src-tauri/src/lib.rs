//! TrainLink desktop core. The Rust side owns the database, the Z21 link, and the API the web UI calls.
//! The UI calls the API at trainlink://localhost, so its calls are the same as they were against the web backend


pub mod anyrail;
pub mod auth;
pub mod api;
pub mod db;
pub mod fields;
pub mod http;
pub mod import_file;
pub mod library;
pub mod moving_time;
pub mod server;
pub mod sync;
pub mod state;
pub mod z21;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use tauri::http::{Request, Response};
use tauri::{Manager, UriSchemeContext, UriSchemeResponder};

use crate::api::Call;
use crate::state::AppState;

/// The pages the app itself loads, by platform: macOS and Linux use tauri://, Windows uses http://tauri.localhost
const APP_ORIGINS: [&str; 3] = ["tauri://localhost", "http://tauri.localhost", "https://tauri.localhost"];

/// Answer one request from the UI. CORS headers let the app's own pages call the API, and no other origin
fn answer(app: &Arc<AppState>, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let content_type = request
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let origin = request
        .headers()
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .filter(|origin| APP_ORIGINS.contains(origin))
        .map(str::to_string);
    let call = Call {
        method: request.method().as_str(),
        path: request.uri().path(),
        query: request.uri().query().unwrap_or_default(),
        body: request.body(),
        content_type,
    };
    let reply = api::handle(app, &call);
    let mut response = Response::builder()
        .status(reply.status)
        .header("Content-Type", reply.content_type)
        .header("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type, Authorization")
        .header("Vary", "Origin");
    if let Some(origin) = origin {
        response = response.header("Access-Control-Allow-Origin", origin);
    }
    response
        .body(reply.body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![import_file::pick_anyrail_file, import_file::pick_picture_file])
        .setup(|app| {
            let folder = app.path().app_data_dir()?;
            std::fs::create_dir_all(&folder)?;
            let connection = db::open(&folder.join("trainlink.db"))?;
            let state = Arc::new(AppState::new(connection, folder.join("locomotive_images")));
            sync::spawn_loop(state.clone());
            app.manage(state);
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("trainlink", |context: UriSchemeContext<'_, _>, request, responder: UriSchemeResponder| {
            let app = context.app_handle().clone();
            std::thread::spawn(move || {
                let state = app.state::<Arc<AppState>>();
                let response = answer(state.inner(), &request);
                responder.respond(response);
            });
        })
        .run(tauri::generate_context!())
        .expect("error while running TrainLink");
}
