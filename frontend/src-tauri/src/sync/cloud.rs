//! The cloud side of sync: reading, writing, and deleting rows, and storing pictures. The Supabase version calls
//! the project's API as the signed-in user, so row-level security applies. The memory version is for tests

use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;

use serde_json::Value;

use crate::http::ApiError;

pub trait Cloud: Send + Sync {
    /// The signed-in user's id, written into every row they own
    fn user_id(&self) -> String;
    /// Every row of a table the user owns, including rows deleted in the cloud (their deleted_at is set)
    fn fetch(&self, table: &str) -> Result<Vec<Value>, ApiError>;
    /// Insert or replace rows by id
    fn upsert(&self, table: &str, rows: &[Value]) -> Result<(), ApiError>;
    /// Mark rows deleted, so every device learns of the deletion
    fn delete(&self, table: &str, ids: &[String], at: &str) -> Result<(), ApiError>;
    fn upload_picture(&self, path: &str, bytes: &[u8]) -> Result<(), ApiError>;
    fn download_picture(&self, path: &str) -> Result<Vec<u8>, ApiError>;
}

/// Rows the API returns in pages of this size
const PAGE: usize = 1000;
/// Rows sent in one request
const CHUNK: usize = 200;
const PICTURE_BUCKET: &str = "locomotive-pictures";

pub struct SupabaseCloud {
    base: String,
    anon_key: String,
    token: String,
    user_id: String,
    agent: ureq::Agent,
}

impl SupabaseCloud {
    pub fn new(base: &str, anon_key: &str, token: &str, user_id: &str) -> SupabaseCloud {
        SupabaseCloud {
            base: base.trim_end_matches('/').to_string(),
            anon_key: anon_key.to_string(),
            token: token.to_string(),
            user_id: user_id.to_string(),
            agent: ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(20)).build(),
        }
    }

    fn with_auth(&self, request: ureq::Request) -> ureq::Request {
        request
            .set("apikey", &self.anon_key)
            .set("Authorization", &format!("Bearer {}", self.token))
    }

    fn check(result: Result<ureq::Response, ureq::Error>) -> Result<ureq::Response, ApiError> {
        match result {
            Ok(response) => Ok(response),
            Err(ureq::Error::Status(code, response)) => {
                let detail = response.into_string().unwrap_or_default();
                Err(ApiError::new(if code == 401 || code == 403 { 401 } else { 503 }, format!("Sync was refused ({code}): {detail}")))
            }
            Err(ureq::Error::Transport(_)) => Err(ApiError::new(503, "Could not reach the cloud to sync")),
        }
    }
}

impl Cloud for SupabaseCloud {
    fn user_id(&self) -> String {
        self.user_id.clone()
    }

    fn fetch(&self, table: &str) -> Result<Vec<Value>, ApiError> {
        let mut rows = Vec::new();
        loop {
            let from = rows.len();
            let request = self
                .with_auth(self.agent.get(&format!("{}/rest/v1/{table}?select=*&order=id", self.base)))
                .set("Range-Unit", "items")
                .set("Range", &format!("{from}-{}", from + PAGE - 1));
            let page: Vec<Value> = Self::check(request.call())?
                .into_json()
                .map_err(|error| ApiError::internal(format!("Could not read the cloud rows: {error}")))?;
            let count = page.len();
            rows.extend(page);
            if count < PAGE {
                return Ok(rows);
            }
        }
    }

    fn upsert(&self, table: &str, rows: &[Value]) -> Result<(), ApiError> {
        for chunk in rows.chunks(CHUNK) {
            let request = self
                .with_auth(self.agent.post(&format!("{}/rest/v1/{table}?on_conflict=id", self.base)))
                .set("Content-Type", "application/json")
                .set("Prefer", "resolution=merge-duplicates,return=minimal");
            Self::check(request.send_json(Value::Array(chunk.to_vec())))?;
        }
        Ok(())
    }

    fn delete(&self, table: &str, ids: &[String], at: &str) -> Result<(), ApiError> {
        for chunk in ids.chunks(CHUNK) {
            let list = chunk.join(",");
            let request = self
                .with_auth(self.agent.patch(&format!("{}/rest/v1/{table}?id=in.({list})", self.base)))
                .set("Content-Type", "application/json")
                .set("Prefer", "return=minimal");
            Self::check(request.send_json(serde_json::json!({ "deleted_at": at })))?;
        }
        Ok(())
    }

    fn upload_picture(&self, path: &str, bytes: &[u8]) -> Result<(), ApiError> {
        let content_type = if path.ends_with(".png") {
            "image/png"
        } else if path.ends_with(".webp") {
            "image/webp"
        } else {
            "image/jpeg"
        };
        let request = self
            .with_auth(self.agent.post(&format!("{}/storage/v1/object/{PICTURE_BUCKET}/{path}", self.base)))
            .set("Content-Type", content_type)
            .set("x-upsert", "true");
        Self::check(request.send_bytes(bytes))?;
        Ok(())
    }

    fn download_picture(&self, path: &str) -> Result<Vec<u8>, ApiError> {
        let request = self.with_auth(self.agent.get(&format!("{}/storage/v1/object/authenticated/{PICTURE_BUCKET}/{path}", self.base)));
        let mut bytes = Vec::new();
        Self::check(request.call())?
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|error| ApiError::internal(format!("Could not read the picture: {error}")))?;
        Ok(bytes)
    }
}

/// Keeps the cloud in memory, with the same rules as the project: rows are replaced by id, deletions are marked,
/// and each write stamps updated_at. For tests
pub struct MemoryCloud {
    pub user: String,
    tables: Mutex<HashMap<String, HashMap<String, Value>>>,
    pictures: Mutex<HashMap<String, Vec<u8>>>,
    clock: Mutex<u64>,
}

impl MemoryCloud {
    pub fn new(user: &str) -> MemoryCloud {
        MemoryCloud { user: user.into(), tables: Mutex::new(HashMap::new()), pictures: Mutex::new(HashMap::new()), clock: Mutex::new(0) }
    }

    fn stamp(&self) -> String {
        let mut clock = self.clock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *clock += 1;
        format!("{}+00:00", crate::db::now())
    }
}

impl Cloud for MemoryCloud {
    fn user_id(&self) -> String {
        self.user.clone()
    }

    fn fetch(&self, table: &str) -> Result<Vec<Value>, ApiError> {
        let tables = self.tables.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut rows: Vec<Value> = tables.get(table).map(|rows| rows.values().cloned().collect()).unwrap_or_default();
        rows.sort_by_key(|row| row["id"].as_str().unwrap_or_default().to_string());
        Ok(rows)
    }

    fn upsert(&self, table: &str, rows: &[Value]) -> Result<(), ApiError> {
        let mut tables = self.tables.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for row in rows {
            let mut row = row.clone();
            row["updated_at"] = Value::String(self.stamp());
            let id = row["id"].as_str().unwrap_or_default().to_string();
            tables.entry(table.to_string()).or_default().insert(id, row);
        }
        Ok(())
    }

    fn delete(&self, table: &str, ids: &[String], at: &str) -> Result<(), ApiError> {
        let mut tables = self.tables.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for id in ids {
            if let Some(row) = tables.get_mut(table).and_then(|rows| rows.get_mut(id)) {
                row["deleted_at"] = Value::String(at.to_string());
                row["updated_at"] = Value::String(self.stamp());
            }
        }
        Ok(())
    }

    fn upload_picture(&self, path: &str, bytes: &[u8]) -> Result<(), ApiError> {
        self.pictures.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(path.to_string(), bytes.to_vec());
        Ok(())
    }

    fn download_picture(&self, path: &str) -> Result<Vec<u8>, ApiError> {
        self.pictures
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(path)
            .cloned()
            .ok_or_else(|| ApiError::not_found_message("Picture not found in the cloud"))
    }
}
