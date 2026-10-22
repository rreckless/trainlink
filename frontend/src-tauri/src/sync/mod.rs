//! Sync between this device and the cloud. Each run: read the cloud, apply its changes (the newer change wins),
//! send this device's changes, and mark what was sent as clean. Network calls never hold the database lock.

pub mod cloud;
pub mod events;
pub mod map;
pub mod settings;
pub mod time;
pub mod trackside;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;

use crate::auth::{SUPABASE_ANON_KEY, SUPABASE_URL};
use crate::db::sync as bookkeeping;
use crate::http::ApiError;
use crate::state::AppState;
use cloud::{Cloud, SupabaseCloud};

/// Parents before children, for applying and sending
const TABLES: [&str; 7] = ["layouts", "locomotives", "blocks", "signals", "feedbacks", "routes", "settings"];
const EVENTS: &str = "moving_time_events";
const INTERVAL: Duration = Duration::from_secs(30);
/// How long after a change the loop waits before it reads, so that a burst of edits is sent together
const DEBOUNCE: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Default, Serialize)]
pub struct SyncStatus {
    pub last_synced: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Default, PartialEq)]
pub struct Report {
    pub pulled: usize,
    pub pushed: usize,
}

/// One row to send, and the local stamp it had when it was read, so it is only marked clean if unchanged since
struct Outgoing {
    table: &'static str,
    local_id: i64,
    stamp: String,
    row: Value,
}

/// This device belongs to one account. The first account to sign in claims it; another account is refused
pub fn claim(app: &AppState, user: &str) -> Result<(), ApiError> {
    let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match bookkeeping::meta(&inner.db, "owner")? {
        None => bookkeeping::set_meta(&inner.db, "owner", user).map_err(ApiError::from),
        Some(owner) if owner == user => Ok(()),
        Some(_) => Err(ApiError::conflict("This Mac already belongs to another account. Sign in to that account instead")),
    }
}

/// Run one sync for the signed-in account
pub fn run_for_session(app: &AppState) -> Result<Report, ApiError> {
    let session = app.auth.session().ok_or_else(|| ApiError::new(401, "Sign in to sync"))?;
    claim(app, &session.user_id)?;
    let cloud = SupabaseCloud::new(SUPABASE_URL, SUPABASE_ANON_KEY, &session.access_token, &session.user_id);
    let images = app.images.clone();
    run(app, &cloud, &images)
}

/// Sync now, and remember the outcome for the status
pub fn sync_and_record(app: &AppState) -> Result<Report, ApiError> {
    let result = run_for_session(app);
    let mut status = app.sync.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match &result {
        Ok(_) => {
            status.last_synced = Some(crate::db::now());
            status.last_error = None;
        }
        Err(error) => status.last_error = Some(error.detail.clone()),
    }
    result
}

/// Set when a change has been made that should be sent. The loop wakes on it, instead of waiting for the interval
static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// Asks the sync loop to run soon. Several changes close together make one sync, because the loop waits DEBOUNCE
/// after the first before it reads the database
pub fn request_sync() {
    let (pending, wake) = &WAKE;
    *pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    wake.notify_one();
}

/// Waits until a sync is requested, or the timeout passes. Clears the request
pub fn wait_for_request(timeout: Duration) {
    let (pending, wake) = &WAKE;
    let guard = pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (mut guard, _) = wake
        .wait_timeout_while(guard, timeout, |requested| !*requested)
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = false;
}

/// Sync while signed in: soon after a change, and every interval as well, which picks up changes from other devices.
/// Failures are recorded, and the next run tries again
pub fn spawn_loop(app: Arc<AppState>) {
    std::thread::spawn(move || loop {
        wait_for_request(INTERVAL);
        std::thread::sleep(DEBOUNCE);
        if app.auth.session().is_some() {
            let _ = sync_and_record(&app);
        }
    });
}

pub fn run(app: &AppState, cloud: &dyn Cloud, images: &Path) -> Result<Report, ApiError> {
    let user = cloud.user_id();

    // 1. Read the cloud. No lock is held
    let mut remote: HashMap<&str, Vec<Value>> = HashMap::new();
    for table in TABLES.iter().copied().chain(std::iter::once(EVENTS)) {
        remote.insert(table, cloud.fetch(table)?);
    }

    // 2. Apply the cloud's changes, and collect this device's, under the lock
    let mut report = Report::default();
    let mut downloads: Vec<(i64, String)> = Vec::new();
    let mut uploads: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut outgoing: Vec<Outgoing> = Vec::new();
    let mut pending: Vec<(i64, &'static str, String)> = Vec::new();
    let event_rows: Vec<(i64, String, Value)>;
    {
        let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let db = &inner.db;
        apply_deletions(db, &remote, images)?;
        for table in TABLES {
            for row in remote[table].iter().filter(|row| row.get("deleted_at").is_none_or(Value::is_null)) {
                let applied = match table {
                    "layouts" => map::apply_layout(db, row)?,
                    "locomotives" => map::apply_locomotive(db, row, images, &mut downloads)?,
                    "blocks" => trackside::apply_block(db, row)?,
                    "signals" => trackside::apply_signal(db, row)?,
                    "feedbacks" => trackside::apply_feedback(db, row)?,
                    "settings" => settings::apply_setting(db, row)?,
                    _ => trackside::apply_route(db, row)?,
                };
                report.pulled += usize::from(applied);
            }
        }
        for row in &remote[EVENTS] {
            report.pulled += usize::from(events::apply(db, row)?);
        }

        // Our own changes: every row gets its cloud id first, so references can be written
        for table in TABLES {
            map::ensure_sync_ids(db, table)?;
        }
        for (table, rows) in [
            ("layouts", map::layout_outbound(db, &user)?),
            ("locomotives", map::locomotive_outbound(db, &user, images, &mut uploads)?),
            ("blocks", trackside::block_outbound(db, &user)?),
            ("signals", trackside::signal_outbound(db, &user)?),
            ("feedbacks", trackside::feedback_outbound(db, &user)?),
            ("routes", trackside::route_outbound(db, &user)?),
            ("settings", settings::setting_outbound(db, &user)?),
        ] {
            for (local_id, stamp, row) in rows {
                outgoing.push(Outgoing { table, local_id, stamp, row });
            }
        }
        let events_out = events::outbound(db, &user)?;
        event_rows = events_out;
        let mut statement = db.prepare("SELECT id, table_name, sync_id FROM pending_deletes ORDER BY id")?;
        for (id, table, sync_id) in statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
        {
            if let Some(table) = TABLES.iter().find(|known| **known == table) {
                pending.push((id, table, sync_id));
            }
        }
    }

    // 3. Send this device's changes. No lock is held
    for (path, file) in &uploads {
        if let Ok(bytes) = std::fs::read(file) {
            cloud.upload_picture(path, &bytes)?;
        }
    }
    for table in TABLES {
        let rows: Vec<Value> = outgoing.iter().filter(|item| item.table == table).map(|item| item.row.clone()).collect();
        if !rows.is_empty() {
            cloud.upsert(table, &rows)?;
            report.pushed += rows.len();
        }
    }
    let events_rows: Vec<Value> = event_rows.iter().map(|(_, _, row)| row.clone()).collect();
    if !events_rows.is_empty() {
        cloud.upsert(EVENTS, &events_rows)?;
        report.pushed += events_rows.len();
    }
    let now = crate::db::now();
    for table in TABLES {
        let ids: Vec<String> = pending.iter().filter(|(_, name, _)| *name == table).map(|(_, _, sync)| sync.clone()).collect();
        if !ids.is_empty() {
            cloud.delete(table, &ids, &time::to_cloud(&now))?;
        }
    }

    // Pictures this device is missing
    let mut fetched = Vec::new();
    for (id, path) in downloads {
        if let Ok(bytes) = cloud.download_picture(&path) {
            fetched.push((id, path, bytes));
        }
    }

    // 4. Mark what was sent as clean, and keep what was downloaded. Changes made during the network calls stay dirty
    {
        let inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let db = &inner.db;
        for item in &outgoing {
            mark_pushed(db, item.table, item.local_id, &item.stamp)?;
        }
        for (id, _, _) in &event_rows {
            db.execute("UPDATE moving_time_events SET dirty = 0 WHERE id = ?1", [id])?;
        }
        for (id, _, _) in &pending {
            db.execute("DELETE FROM pending_deletes WHERE id = ?1", [id])?;
        }
        for (id, path, bytes) in fetched {
            let extension = path.rsplit('.').next().unwrap_or("jpg").to_string();
            let file = format!("{id}.{extension}");
            std::fs::create_dir_all(images).ok();
            if std::fs::write(images.join(&file), &bytes).is_ok() {
                db.execute("UPDATE locomotives SET image_file = ?2 WHERE id = ?1", params![id, file])?;
            }
        }
        bookkeeping::set_meta(db, "last_sync", &now)?;
    }
    Ok(report)
}

/// Remove rows the cloud has deleted: children before parents, so no row is left pointing at nothing
fn apply_deletions(db: &Connection, remote: &HashMap<&str, Vec<Value>>, images: &Path) -> Result<(), ApiError> {
    for table in TABLES.iter().rev() {
        for row in remote[table].iter().filter(|row| row.get("deleted_at").is_some_and(|value| !value.is_null())) {
            let Some(sync_id) = row.get("id").and_then(Value::as_str) else { continue };
            let Some(local) = map::local_id_of(db, table, sync_id)? else { continue };
            match *table {
                "layouts" => {
                    for child in ["routes", "feedbacks", "signals", "blocks"] {
                        db.execute(&format!("DELETE FROM {child} WHERE layout_id = ?1"), [local])?;
                    }
                }
                "locomotives" => {
                    if let Some(file) = db.query_row("SELECT image_file FROM locomotives WHERE id = ?1", [local], |row| row.get::<_, Option<String>>(0))? {
                        let _ = std::fs::remove_file(images.join(file));
                    }
                    db.execute("UPDATE blocks SET occupancy_locomotive_id = NULL WHERE occupancy_locomotive_id = ?1", [local])?;
                    db.execute("UPDATE routes SET current_locomotive_id = NULL WHERE current_locomotive_id = ?1", [local])?;
                }
                _ => {}
            }
            db.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [local])?;
        }
    }
    Ok(())
}

/// Mark a row clean once it has been sent, unless it changed while the send was in progress
fn mark_pushed(db: &Connection, table: &str, id: i64, stamp: &str) -> Result<(), ApiError> {
    db.execute(&format!("UPDATE {table} SET dirty = 0 WHERE id = ?1 AND updated_at = ?2"), params![id, stamp])?;
    Ok(())
}
