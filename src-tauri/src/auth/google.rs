//! Sign-in with Google, through Supabase Auth. The browser goes to Google and comes back with a code. The app keeps a
//! secret (the verifier) and gives Supabase only its hash, so the code can be exchanged only by the app that started
//! the sign-in (PKCE). The desktop app listens on this computer for the browser to come back; the web server answers
//! the browser at its own address.

use std::io::BufReader;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{Auth, Session};
use crate::http::{ApiError, Reply};
use crate::server::{read_request, write_reply};
use crate::state::AppState;

/// The port the desktop app listens on for the browser to come back to. It must be on Supabase's redirect list
pub const DESKTOP_PORT: u16 = 47831;
pub const DESKTOP_RETURN: &str = "http://127.0.0.1:47831/callback";
/// The link the Android app opens when Google returns. The system browser opens it, and the app takes it over
pub const APP_RETURN: &str = "trainlink://auth-callback";

/// Set once the desktop listener is running, so that it starts only once
static LISTENING: AtomicBool = AtomicBool::new(false);

/// The secret a sign-in is bound to: 64 random hex digits
fn new_verifier() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// The hash of the verifier, which is the only part Supabase sees when the sign-in starts
pub fn challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Percent-encodes a query value
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

impl Auth {
    /// The address that starts a Google sign-in. The browser comes back to return_to, with a code
    pub fn google_start(&self, return_to: &str) -> String {
        let verifier = new_verifier();
        let url = format!(
            "{}/auth/v1/authorize?provider=google&redirect_to={}&code_challenge={}&code_challenge_method=s256",
            self.base,
            encode(return_to),
            challenge(&verifier)
        );
        *self.pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(verifier);
        url
    }

    /// Signs in with the code the browser brought back. Only a sign-in this app started can use it. A refused code
    /// keeps the sign-in waiting, so that a forged return cannot cancel the real one
    pub fn google_finish(&self, code: &str) -> Result<Session, ApiError> {
        let verifier = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .ok_or_else(|| ApiError::bad_request("Start the Google sign-in again"))?;
        let reply = self.post("/auth/v1/token?grant_type=pkce", None, json!({ "auth_code": code, "code_verifier": verifier }))?;
        let session = super::session_from(&reply)?;
        *self.pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        self.save(&session)?;
        Ok(session)
    }
}

/// The code in a callback's query, if it is well formed. Anything else is refused before it reaches the service
fn code_from(query: &str) -> Option<&str> {
    query.split('&').find_map(|pair| pair.strip_prefix("code=")).filter(|code| well_formed_code(code))
}

/// Whether a sign-in code has the shape Supabase gives, so that nothing else reaches the service
pub fn well_formed_code(code: &str) -> bool {
    !code.is_empty() && code.len() <= 512 && code.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

/// Completes a sign-in whose return the app received itself, from its own link. The code is checked first
pub fn complete(app: &Arc<AppState>, code: &str) -> Result<(), ApiError> {
    if !well_formed_code(code) {
        return Err(ApiError::bad_request("The sign-in code was not valid"));
    }
    finish(app, code)
}

/// The browser's return from Google. Signs the device in with the code, claims it for the account, and shows the outcome
pub fn callback(app: &Arc<AppState>, query: &str, continue_in_browser: bool) -> Reply {
    let outcome = match code_from(query) {
        Some(code) => finish(app, code),
        None => Err(ApiError::bad_request("Google sign-in was cancelled or did not finish")),
    };
    page(outcome, continue_in_browser)
}

fn finish(app: &Arc<AppState>, code: &str) -> Result<(), ApiError> {
    let session = app.auth.google_finish(code)?;
    crate::api::auth::claim_or_sign_out(app, &session.user_id)
}

fn page(outcome: Result<(), ApiError>, continue_in_browser: bool) -> Reply {
    let body = match outcome {
        Ok(()) if continue_in_browser => {
            "<!doctype html><meta charset=\"utf-8\"><meta http-equiv=\"refresh\" content=\"0; url=/\"><title>TrainLink</title><p>Signed in. Taking you to TrainLink…</p>"
                .to_string()
        }
        Ok(()) => "<!doctype html><meta charset=\"utf-8\"><title>TrainLink</title><p>Signed in. You can close this tab and return to TrainLink.</p>".to_string(),
        Err(error) => format!(
            "<!doctype html><meta charset=\"utf-8\"><title>TrainLink</title><p>Sign-in did not finish: {}</p><p>Close this tab and try again in TrainLink.</p>",
            escape(&error.detail)
        ),
    };
    Reply::file("text/html; charset=utf-8", body.into_bytes())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Starts the listener the desktop app's browser returns to. It runs for as long as the app does
pub fn listen(app: &Arc<AppState>) -> Result<(), ApiError> {
    if LISTENING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let listener = match TcpListener::bind(("127.0.0.1", DESKTOP_PORT)) {
        Ok(listener) => listener,
        Err(_) => {
            LISTENING.store(false, Ordering::SeqCst);
            return Err(ApiError::internal(format!("Port {DESKTOP_PORT} is in use, so Google sign-in cannot return to TrainLink")));
        }
    };
    let app = app.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            serve(&app, stream);
        }
    });
    Ok(())
}

/// Answers one return from the browser. Only requests addressed to this computer are answered
fn serve(app: &Arc<AppState>, stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(clone) = stream.try_clone() else { return };
    let mut out = stream;
    let reply = match read_request(&mut BufReader::new(clone)) {
        Ok(request) if matches!(request.header("host"), Some("127.0.0.1:47831") | Some("localhost:47831")) => {
            let (path, query) = request.target.split_once('?').unwrap_or((request.target.as_str(), ""));
            if path == "/callback" {
                callback(app, query, false)
            } else {
                Reply::json(404, json!({ "detail": "Not found" }))
            }
        }
        _ => Reply::json(400, json!({ "detail": "This is not a return from a sign-in" })),
    };
    let _ = write_reply(&mut out, &reply, None, None);
}
