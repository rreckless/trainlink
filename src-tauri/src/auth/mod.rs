//! Sign-in with Supabase Auth. The session (access and refresh tokens, and who it belongs to) is kept in the
//! operating system's keychain. A session that cannot be refreshed because the network is down is kept, so the
//! app still works offline with its local data.

pub mod store;

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::http::ApiError;
use store::SessionStore;

/// The Supabase project for TrainLink (West-EU). The anon key is public by design: row-level security protects
/// the data, so this key does not need to be secret. It must never be the secret or service key.
pub const SUPABASE_URL: &str = "https://bnvvdicbimmmcmllease.supabase.co";
pub const SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImJudnZkaWNiaW1tbWNtbGxlYXNlIiwicm9sZSI6ImFub24iLCJpYXQiOjE3OTI2NTg2ODEsImV4cCI6MjEwODIzNDY4MX0.JoedYTnM46dhLwCZdU4NzJeS91I3vMbDq6ee4FilTo0";

/// A session is refreshed this many seconds before its access token expires
const REFRESH_MARGIN_SECS: i64 = 60;
pub const MIN_PASSWORD_LENGTH: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix time, in seconds, when the access token expires
    pub expires_at: i64,
    pub user_id: String,
    pub email: String,
}

/// What signing up did: a session, or a request to confirm the email first
#[derive(Debug, PartialEq)]
pub enum SignUp {
    SignedIn(Session),
    ConfirmEmail,
}

pub struct Auth {
    base: String,
    anon_key: String,
    pub(crate) store: Box<dyn SessionStore>,
    agent: ureq::Agent,
    /// One refresh at a time, so two requests don't both spend the same refresh token
    refreshing: Mutex<()>,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

/// Check an email and a password before they are sent
pub fn check_credentials(email: &str, password: &str) -> Result<(), ApiError> {
    let email = email.trim();
    if email.len() > 254 || !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return Err(ApiError::bad_request("Enter a valid email address"));
    }
    if password.chars().count() < MIN_PASSWORD_LENGTH {
        return Err(ApiError::bad_request(format!("The password must be at least {MIN_PASSWORD_LENGTH} characters")));
    }
    Ok(())
}

impl Auth {
    pub fn new(base: &str, anon_key: &str, store: Box<dyn SessionStore>) -> Auth {
        Auth {
            base: base.trim_end_matches('/').to_string(),
            anon_key: anon_key.to_string(),
            store,
            agent: ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(15)).build(),
            refreshing: Mutex::new(()),
        }
    }

    /// The Supabase project, with the keychain for the session
    pub fn for_app() -> Auth {
        Auth::new(SUPABASE_URL, SUPABASE_ANON_KEY, Box::new(store::KeychainStore::new()))
    }

    /// The session to use now, refreshed if it is about to expire. None if nobody is signed in.
    /// A refresh the server refuses signs the device out. A refresh that fails because the network is down keeps
    /// the stored session, so the app works offline.
    pub fn session(&self) -> Option<Session> {
        let stored = self.store.load()?;
        if stored.expires_at - now() > REFRESH_MARGIN_SECS {
            return Some(stored);
        }
        let _guard = self.refreshing.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // Another request may have refreshed while this one waited
        let stored = self.store.load()?;
        if stored.expires_at - now() > REFRESH_MARGIN_SECS {
            return Some(stored);
        }
        match self.request_token("refresh_token", json!({ "refresh_token": stored.refresh_token })) {
            Ok(session) => {
                let _ = self.store.save(&session);
                Some(session)
            }
            Err(error) if error.status == 503 => Some(stored),
            Err(_) => {
                self.store.clear();
                None
            }
        }
    }

    pub fn sign_in(&self, email: &str, password: &str) -> Result<Session, ApiError> {
        check_credentials(email, password)?;
        let session = self.request_token("password", json!({ "email": email.trim(), "password": password }))?;
        self.save(&session)?;
        Ok(session)
    }

    pub fn sign_up(&self, email: &str, password: &str) -> Result<SignUp, ApiError> {
        check_credentials(email, password)?;
        let body = json!({ "email": email.trim(), "password": password });
        let reply = self.post("/auth/v1/signup", None, body)?;
        if reply.get("access_token").is_some() {
            let session = session_from(&reply)?;
            self.save(&session)?;
            return Ok(SignUp::SignedIn(session));
        }
        Ok(SignUp::ConfirmEmail)
    }

    pub fn sign_out(&self) {
        if let Some(session) = self.store.load() {
            let _ = self.post("/auth/v1/logout", Some(&session.access_token), json!({}));
        }
        self.store.clear();
    }

    pub fn reset_password(&self, email: &str) -> Result<(), ApiError> {
        let email = email.trim();
        if email.is_empty() || !email.contains('@') {
            return Err(ApiError::bad_request("Enter a valid email address"));
        }
        self.post("/auth/v1/recover", None, json!({ "email": email }))?;
        Ok(())
    }

    fn save(&self, session: &Session) -> Result<(), ApiError> {
        self.store.save(session).map_err(|detail| ApiError::internal(format!("Could not keep the sign-in: {detail}")))
    }

    fn request_token(&self, grant: &str, body: Value) -> Result<Session, ApiError> {
        let reply = self.post(&format!("/auth/v1/token?grant_type={grant}"), None, body)?;
        session_from(&reply)
    }

    /// POST to the Supabase Auth API. Refused requests give the service's message; a network failure gives 503
    fn post(&self, path: &str, bearer: Option<&str>, body: Value) -> Result<Value, ApiError> {
        let mut request = self
            .agent
            .post(&format!("{}{}", self.base, path))
            .set("apikey", &self.anon_key)
            .set("Content-Type", "application/json");
        if let Some(token) = bearer {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        match request.send_json(body) {
            Ok(response) => Ok(response.into_json::<Value>().unwrap_or(Value::Null)),
            Err(ureq::Error::Status(_, response)) => {
                let body = response.into_json::<Value>().unwrap_or(Value::Null);
                let message = ["msg", "error_description", "message", "error"]
                    .iter()
                    .find_map(|key| body.get(*key).and_then(Value::as_str))
                    .unwrap_or("The sign-in service refused the request");
                Err(ApiError::new(refusal_status(&body), message))
            }
            Err(ureq::Error::Transport(_)) => Err(ApiError::new(503, "Could not reach the sign-in service")),
        }
    }
}

/// Refused sign-ins are shown as bad requests, except a missing confirmation, which the user must act on
fn refusal_status(body: &Value) -> u16 {
    match body.get("error_code").and_then(Value::as_str) {
        Some("email_not_confirmed") => 403,
        _ => 400,
    }
}

fn session_from(reply: &Value) -> Result<Session, ApiError> {
    let access_token = reply.get("access_token").and_then(Value::as_str);
    let refresh_token = reply.get("refresh_token").and_then(Value::as_str);
    let user = reply.get("user");
    let (Some(access_token), Some(refresh_token), Some(user_id), Some(email)) = (
        access_token,
        refresh_token,
        user.and_then(|user| user.get("id")).and_then(Value::as_str),
        user.and_then(|user| user.get("email")).and_then(Value::as_str),
    ) else {
        return Err(ApiError::internal("The sign-in service sent an unexpected reply"));
    };
    let expires_in = reply.get("expires_in").and_then(Value::as_i64).unwrap_or(3600);
    Ok(Session {
        access_token: access_token.to_string(),
        refresh_token: refresh_token.to_string(),
        expires_at: now() + expires_in,
        user_id: user_id.to_string(),
        email: email.to_string(),
    })
}
