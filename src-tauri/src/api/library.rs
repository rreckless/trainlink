//! The track library, read-only

use serde_json::json;

use super::Call;
use crate::http::{ApiError, Out, Reply};
use crate::library::library;

pub fn route(call: &Call, rest: &[&str]) -> Out {
    match (call.method, rest) {
        ("GET", []) => {
            let pieces = library().list(
                call.param("system").as_deref(),
                call.param("gauge").as_deref(),
                call.param("manufacturer").as_deref(),
            );
            Ok(Reply::json(200, json!(pieces)))
        }
        ("GET", [piece_id]) => match library().get(piece_id) {
            Some(piece) => Ok(Reply::json(200, piece.clone())),
            None => Err(ApiError::not_found_message(format!("Track piece {piece_id} not found"))),
        },
        _ => Err(ApiError::new(405, "Method not allowed")),
    }
}
