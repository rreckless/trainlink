//! TrainLink desktop core. The Rust side owns the database, the Z21 link, and the API the web UI calls.
//! The UI calls the API at trainlink://localhost, so its calls are the same as they were against the web backend


pub mod anyrail;
pub mod api;
pub mod db;
pub mod fields;
pub mod http;
pub mod library;
pub mod moving_time;
pub mod server;
pub mod state;
pub mod z21;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use tauri::http::{Request, Response};
use tauri::{Manager, UriSchemeContext, UriSchemeResponder};

use crate::api::Call;
use crate::state::AppState;

/// Answer one request from the UI. CORS headers let the UI's own origin call the API
fn answer(app: &Arc<AppState>, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let content_type = request
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let call = Call {
        method: request.method().as_str(),
        path: request.uri().path(),
        query: request.uri().query().unwrap_or_default(),
        body: request.body(),
        content_type,
    };
    let reply = api::handle(app, &call);
    Response::builder()
        .status(reply.status)
        .header("Content-Type", reply.content_type)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "*")
        .body(reply.body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let folder = app.path().app_data_dir()?;
            std::fs::create_dir_all(&folder)?;
            let connection = db::open(&folder.join("trainlink.db"))?;
            app.manage(Arc::new(AppState::new(connection, folder.join("locomotive_images"))));
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
