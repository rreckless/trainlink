//! Sign-in against a fake Supabase Auth server on this machine. No request goes to the real service

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;

use serde_json::{json, Value};

use super::send;
use crate::auth::store::MemoryStore;
use crate::auth::{check_credentials, Auth, Session, SignUp};
use crate::state::AppState;

/// Answers each request with the reply the test gives for its path and query
fn fake_service(reply: fn(&str, &Value) -> (u16, Value)) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let target = line.split_whitespace().nth(1).unwrap_or_default().to_string();
            let mut length = 0usize;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header.trim().is_empty() {
                    break;
                }
                if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; length];
            reader.read_exact(&mut body).unwrap();
            let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let (status, reply_body) = reply(&target, &body);
            let text = reply_body.to_string();
            let mut out = stream;
            write!(out, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len()).unwrap();
        }
    });
    base
}

fn session_reply(email: &str, user: &str, access: &str, refresh: &str) -> Value {
    json!({
        "access_token": access,
        "refresh_token": refresh,
        "expires_in": 3600,
        "user": { "id": user, "email": email },
    })
}

fn auth_with(base: &str) -> Auth {
    Auth::new(base, "test-anon-key", Box::new(MemoryStore::new()))
}

fn expiring(auth: &Auth, expires_at: i64) {
    let session = Session {
        access_token: "old-access".into(),
        refresh_token: "old-refresh".into(),
        expires_at,
        user_id: "user-1".into(),
        email: "a@example.com".into(),
    };
    auth.store.save(&session).unwrap();
}

#[test]
fn credentials_are_checked_before_they_are_sent() {
    assert!(check_credentials("not-an-email", "longenough").is_err());
    assert!(check_credentials("a@example.com", "short").is_err());
    assert!(check_credentials("a@example.com", "longenough").is_ok());
}

#[test]
fn signing_in_keeps_the_session() {
    let base = fake_service(|target, body| {
        assert!(target.starts_with("/auth/v1/token?grant_type=password"));
        assert_eq!(body["email"], "a@example.com");
        (200, session_reply("a@example.com", "user-1", "access-1", "refresh-1"))
    });
    let auth = auth_with(&base);
    let session = auth.sign_in("a@example.com", "longenough").unwrap();
    assert_eq!(session.user_id, "user-1");
    assert_eq!(auth.session().unwrap().access_token, "access-1");
}

#[test]
fn a_wrong_password_gives_the_service_message() {
    let base = fake_service(|_, _| (400, json!({ "code": 400, "error_code": "invalid_credentials", "msg": "Invalid login credentials" })));
    let auth = auth_with(&base);
    let error = auth.sign_in("a@example.com", "wrongpassword").unwrap_err();
    assert_eq!((error.status, error.detail.as_str()), (400, "Invalid login credentials"));
    assert!(auth.session().is_none());
}

#[test]
fn an_unconfirmed_email_is_told_to_confirm() {
    let base = fake_service(|_, _| (400, json!({ "code": 400, "error_code": "email_not_confirmed", "msg": "Email not confirmed" })));
    let error = auth_with(&base).sign_in("a@example.com", "longenough").unwrap_err();
    assert_eq!(error.status, 403);
}

#[test]
fn signing_up_without_a_session_asks_for_confirmation() {
    let base = fake_service(|_, _| (200, json!({ "id": "user-2", "email": "b@example.com" })));
    let auth = auth_with(&base);
    assert_eq!(auth.sign_up("b@example.com", "longenough").unwrap(), SignUp::ConfirmEmail);
    assert!(auth.session().is_none());
}

#[test]
fn an_expiring_session_is_refreshed() {
    let base = fake_service(|target, body| {
        assert!(target.contains("grant_type=refresh_token"));
        assert_eq!(body["refresh_token"], "old-refresh");
        (200, session_reply("a@example.com", "user-1", "access-2", "refresh-2"))
    });
    let auth = auth_with(&base);
    expiring(&auth, 0);
    assert_eq!(auth.session().unwrap().access_token, "access-2");
    assert_eq!(auth.store.load().unwrap().refresh_token, "refresh-2");
}

#[test]
fn a_refresh_the_service_refuses_signs_the_device_out() {
    let base = fake_service(|_, _| (400, json!({ "error": "invalid_grant", "error_description": "Invalid Refresh Token" })));
    let auth = auth_with(&base);
    expiring(&auth, 0);
    assert!(auth.session().is_none());
    assert!(auth.store.load().is_none());
}

#[test]
fn offline_the_stored_session_is_kept() {
    // Nothing listens on this port, so the refresh cannot reach the service
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let auth = auth_with(&format!("http://127.0.0.1:{port}"));
    expiring(&auth, 0);
    let kept = auth.session().expect("still signed in while offline");
    assert_eq!(kept.access_token, "old-access");
}

#[test]
fn sign_out_clears_the_session() {
    let base = fake_service(|_, _| (204, Value::Null));
    let auth = auth_with(&base);
    expiring(&auth, i64::MAX / 2);
    auth.sign_out();
    assert!(auth.session().is_none());
}

#[test]
fn the_sign_in_endpoints_answer_on_the_api() {
    let base = fake_service(|_, body| {
        if body["password"] == "longenough" {
            (200, session_reply("a@example.com", "user-1", "access-1", "refresh-1"))
        } else {
            (400, json!({ "msg": "Invalid login credentials" }))
        }
    });
    let app = Arc::new(AppState::with_auth(crate::db::open_in_memory().unwrap(), std::env::temp_dir(), auth_with(&base)));
    let (status, value) = send(&app, "GET", "/api/auth/session", None);
    assert_eq!((status, value["signed_in"].as_bool()), (200, Some(false)));

    let (status, value) = send(&app, "POST", "/api/auth/sign-in", Some(json!({ "email": "a@example.com", "password": "wrongpassword" })));
    assert_eq!(status, 400);
    assert_eq!(value["detail"], "Invalid login credentials");

    let (status, value) = send(&app, "POST", "/api/auth/sign-in", Some(json!({ "email": "a@example.com", "password": "longenough" })));
    assert_eq!(status, 200);
    assert_eq!(value["email"], "a@example.com");
    assert!(value.get("access_token").is_none(), "tokens are never sent to the page");

    let (status, _) = send(&app, "POST", "/api/auth/sign-out", None);
    assert_eq!(status, 200);
    let (_, value) = send(&app, "GET", "/api/auth/session", None);
    assert_eq!(value["signed_in"], false);
}

/// Signs in to the real project with the test account from the environment. Not run by default:
///   set -a; source ~/trainlink-test.env; set +a; cargo test real_sign_in -- --ignored
#[test]
#[ignore]
fn real_sign_in() {
    let email = std::env::var("TEST_A_EMAIL").expect("TEST_A_EMAIL");
    let password = std::env::var("TEST_A_PASSWORD").expect("TEST_A_PASSWORD");
    let auth = Auth::new(crate::auth::SUPABASE_URL, crate::auth::SUPABASE_ANON_KEY, Box::new(MemoryStore::new()));
    let session = auth.sign_in(&email, &password).expect("sign in to the real project");
    assert_eq!(session.email.to_lowercase(), email.to_lowercase());
    assert_eq!(auth.session().expect("still signed in").user_id, session.user_id);
    auth.sign_out();
    assert!(auth.session().is_none());
}

#[test]
fn the_session_file_is_owner_only_and_round_trips() {
    use crate::auth::store::{FileStore, SessionStore};
    let folder = std::env::temp_dir().join(format!("trainlink-session-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    let store = FileStore::new(&folder.join("session.json"));
    let session = Session {
        access_token: "a".into(),
        refresh_token: "r".into(),
        expires_at: 42,
        user_id: "u".into(),
        email: "e@example.com".into(),
    };
    store.save(&session).unwrap();
    assert_eq!(store.load().unwrap(), session);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(folder.join("session.json")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "only the owner can read the session");
    }
    store.clear();
    assert!(store.load().is_none());
}
