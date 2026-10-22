//! Sync bookkeeping in the local database: each synced row's cloud id, which rows have changed, which rows were
//! deleted, and the device's identity. The rest of the sync is in `sync/`

use rusqlite::{params, Connection, OptionalExtension};

/// Tables that sync, in the order a full pull must apply them (parents before children)
pub const SYNCED_TABLES: [&str; 7] = ["layouts", "locomotives", "blocks", "signals", "feedbacks", "routes", "settings"];

pub fn new_sync_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Add the sync columns and tables to a database created before sync existed. Existing rows start as changed,
/// so the first signed-in account claims them
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    for table in SYNCED_TABLES {
        let columns: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info({table})"))?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<_>>()?;
        if !columns.iter().any(|column| column == "sync_id") {
            conn.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN sync_id TEXT; ALTER TABLE {table} ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;"
            ))?;
        }
        conn.execute_batch(&format!("CREATE UNIQUE INDEX IF NOT EXISTS {table}_sync_id ON {table} (sync_id);"))?;
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sync_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS pending_deletes (id INTEGER PRIMARY KEY, table_name TEXT NOT NULL, sync_id TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS moving_time_events (
            id INTEGER PRIMARY KEY,
            sync_id TEXT NOT NULL UNIQUE,
            locomotive_id INTEGER NOT NULL,
            device_id TEXT NOT NULL,
            seconds REAL NOT NULL,
            recorded_at TEXT NOT NULL,
            dirty INTEGER NOT NULL DEFAULT 1
         );",
    )
}

pub fn meta(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM sync_meta WHERE key = ?1", [key], |row| row.get(0)).optional()
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO sync_meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// This device's id, made once and kept
pub fn device_id(conn: &Connection) -> rusqlite::Result<String> {
    if let Some(id) = meta(conn, "device_id")? {
        return Ok(id);
    }
    let id = new_sync_id();
    set_meta(conn, "device_id", &id)?;
    Ok(id)
}

/// A row was just created: give it a cloud id, and mark it changed
pub fn mark_new(conn: &Connection, table: &str, id: i64) -> rusqlite::Result<()> {
    conn.execute(
        &format!("UPDATE {table} SET sync_id = COALESCE(sync_id, ?1), dirty = 1 WHERE id = ?2"),
        params![new_sync_id(), id],
    )?;
    Ok(())
}

/// A row was just changed by the user
pub fn mark_changed(conn: &Connection, table: &str, id: i64) -> rusqlite::Result<()> {
    conn.execute(&format!("UPDATE {table} SET dirty = 1 WHERE id = ?1"), [id])?;
    Ok(())
}

/// A row is about to be deleted: remember its cloud id, so the deletion reaches the cloud
pub fn forget(conn: &Connection, table: &str, id: i64) -> rusqlite::Result<()> {
    let sync_id: Option<String> = conn
        .query_row(&format!("SELECT sync_id FROM {table} WHERE id = ?1"), [id], |row| row.get(0))
        .optional()?
        .flatten();
    if let Some(sync_id) = sync_id {
        conn.execute("INSERT INTO pending_deletes (table_name, sync_id) VALUES (?1, ?2)", params![table, sync_id])?;
    }
    Ok(())
}
