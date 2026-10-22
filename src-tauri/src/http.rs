//! HTTP-shaped replies and errors. The frontend's API calls get these back as status, body, and headers

use serde_json::{json, Value};

/// A reply to one API call
#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn json(status: u16, value: Value) -> Reply {
        Reply {
            status,
            content_type: "application/json".into(),
            body: value.to_string().into_bytes(),
        }
    }

    pub fn empty(status: u16) -> Reply {
        Reply { status, content_type: "application/json".into(), body: Vec::new() }
    }

    pub fn file(content_type: &str, body: Vec<u8>) -> Reply {
        Reply { status: 200, content_type: content_type.into(), body }
    }
}

/// An error that becomes an HTTP error reply with a `detail` message, as FastAPI does
#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    pub status: u16,
    pub detail: String,
}

impl ApiError {
    pub fn new(status: u16, detail: impl Into<String>) -> ApiError {
        ApiError { status, detail: detail.into() }
    }

    pub fn bad_request(detail: impl Into<String>) -> ApiError {
        ApiError::new(400, detail)
    }

    pub fn not_found(label: &str, id: i64) -> ApiError {
        ApiError::new(404, format!("{label} {id} not found"))
    }

    pub fn conflict(detail: impl Into<String>) -> ApiError {
        ApiError::new(409, detail)
    }

    pub fn unprocessable(detail: impl Into<String>) -> ApiError {
        ApiError::new(422, detail)
    }

    pub fn not_found_message(detail: impl Into<String>) -> ApiError {
        ApiError::new(404, detail)
    }

    pub fn internal(detail: impl Into<String>) -> ApiError {
        ApiError::new(500, detail)
    }

    pub fn into_reply(self) -> Reply {
        Reply::json(self.status, json!({ "detail": self.detail }))
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(error: rusqlite::Error) -> ApiError {
        ApiError::internal(format!("Database error: {error}"))
    }
}

pub type Out = Result<Reply, ApiError>;
