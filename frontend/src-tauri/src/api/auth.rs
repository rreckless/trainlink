//! Sign-in endpoints. Passwords are sent to the sign-in service and never stored here

use std::sync::Arc;

use serde_json::{json, Value};

use super::Call;
use crate::auth::SignUp;
use crate::fields;
use crate::http::{ApiError, Out, Reply};
use crate::state::AppState;

pub fn route(app: &Arc<AppState>, call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", ["session"]) => Ok(Reply::json(200, session_json(app))),
        ("POST", ["sign-in"]) => {
            let body = call.json_body()?;
            let email = fields::required_str(&body, "email")?;
            let password = fields::required_str(&body, "password")?;
            let session = app.auth.sign_in(&email, &password)?;
            claim_or_sign_out(app, &session.user_id)?;
            Ok(Reply::json(200, session_json(app)))
        }
        ("POST", ["sign-up"]) => {
            let body = call.json_body()?;
            let email = fields::required_str(&body, "email")?;
            let password = fields::required_str(&body, "password")?;
            let outcome = match app.auth.sign_up(&email, &password)? {
                SignUp::SignedIn(session) => {
                    claim_or_sign_out(app, &session.user_id)?;
                    "signed_in"
                }
                SignUp::ConfirmEmail => "confirm_email",
            };
            Ok(Reply::json(200, json!({ "status": outcome, "session": session_json(app) })))
        }
        ("POST", ["google"]) => {
            let body = call.json_body()?;
            let desktop = body.get("desktop").and_then(Value::as_bool).unwrap_or(false);
            let in_app = body.get("app").and_then(Value::as_bool).unwrap_or(false);
            let return_to = if desktop {
                crate::auth::google::listen(app)?;
                crate::auth::google::DESKTOP_RETURN.to_string()
            } else if in_app {
                crate::auth::google::APP_RETURN.to_string()
            } else {
                let origin = fields::required_str(&body, "origin")?;
                if !is_origin(&origin) {
                    return Err(ApiError::unprocessable("origin must be the address of this TrainLink page"));
                }
                format!("{origin}/auth/callback")
            };
            Ok(Reply::json(200, json!({ "url": app.auth.google_start(&return_to) })))
        }
        ("POST", ["google", "complete"]) => {
            let body = call.json_body()?;
            let code = fields::required_str(&body, "code")?;
            crate::auth::google::complete(app, &code)?;
            Ok(Reply::json(200, session_json(app)))
        }
        ("POST", ["sign-out"]) => {
            app.auth.sign_out();
            Ok(Reply::json(200, session_json(app)))
        }
        ("POST", ["reset-password"]) => {
            let body = call.json_body()?;
            let email = fields::required_str(&body, "email")?;
            app.auth.reset_password(&email)?;
            Ok(Reply::json(200, json!({ "status": "sent" })))
        }
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}

/// A device belongs to one account. If this account may not use it, the new sign-in is undone
pub(crate) fn claim_or_sign_out(app: &Arc<AppState>, user: &str) -> Result<(), ApiError> {
    if let Err(error) = crate::sync::claim(app, user) {
        app.auth.sign_out();
        return Err(error);
    }
    let app_for_sync = app.clone();
    std::thread::spawn(move || {
        let _ = crate::sync::sync_and_record(&app_for_sync);
    });
    Ok(())
}

/// The address of a page, scheme and host only: no path, query, or spaces
fn is_origin(origin: &str) -> bool {
    let rest = origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://"));
    rest.is_some_and(|rest| !rest.is_empty() && rest.len() <= 250 && !rest.contains(['/', '?', '#', ' ', '\\']))
}

/// Who is signed in, if anyone. The tokens are never sent to the page
fn session_json(app: &Arc<AppState>) -> Value {
    match app.auth.session() {
        Some(session) => json!({ "signed_in": true, "email": session.email, "user_id": session.user_id }),
        None => json!({ "signed_in": false }),
    }
}
