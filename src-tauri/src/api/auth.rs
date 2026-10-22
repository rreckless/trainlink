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
            app.auth.sign_in(&email, &password)?;
            Ok(Reply::json(200, session_json(app)))
        }
        ("POST", ["sign-up"]) => {
            let body = call.json_body()?;
            let email = fields::required_str(&body, "email")?;
            let password = fields::required_str(&body, "password")?;
            let outcome = match app.auth.sign_up(&email, &password)? {
                SignUp::SignedIn(_) => "signed_in",
                SignUp::ConfirmEmail => "confirm_email",
            };
            Ok(Reply::json(200, json!({ "status": outcome, "session": session_json(app) })))
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

/// Who is signed in, if anyone. The tokens are never sent to the page
fn session_json(app: &Arc<AppState>) -> Value {
    match app.auth.session() {
        Some(session) => json!({ "signed_in": true, "email": session.email, "user_id": session.user_id }),
        None => json!({ "signed_in": false }),
    }
}
