//! Blocks, signals, feedbacks, and routes. Blocks link to signals and feedbacks through join tables

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::Value;

use super::{now, sync};

#[derive(Debug, Clone, Serialize)]
pub struct Block {
    pub id: i64,
    pub layout_id: i64,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub state: String,
    pub occupied: bool,
    pub max_speed: i64,
    /// The block a train enters after this one, if it is known. Used for caution aspects
    pub next_block_id: Option<i64>,
    pub signal_ids: Vec<i64>,
    pub feedback_ids: Vec<i64>,
    pub created_at: String,
    #[serde(skip)]
    pub length: Option<f64>,
    #[serde(skip)]
    pub occupancy_locomotive_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Signal {
    pub id: i64,
    pub layout_id: i64,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub signal_type: String,
    pub aspect: String,
    pub block_ids: Vec<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Feedback {
    pub id: i64,
    pub layout_id: i64,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub feedback_number: i64,
    pub state: bool,
    pub block_ids: Vec<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Route {
    pub id: i64,
    pub layout_id: i64,
    pub name: String,
    pub block_sequence: Vec<i64>,
    pub speed_profile: Option<Value>,
    pub dwell_times: Option<Value>,
    pub auto_return: bool,
    pub enabled: bool,
    pub is_running: bool,
    pub current_block_index: i64,
    pub created_at: String,
    #[serde(skip)]
    pub current_locomotive_id: Option<i64>,
}

fn json_column(row: &Row, name: &str) -> rusqlite::Result<Option<Value>> {
    let text: Option<String> = row.get(name)?;
    Ok(text.and_then(|value| serde_json::from_str(&value).ok()))
}

fn ids(conn: &Connection, sql: &str, id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map([id], |row| row.get(0))?;
    rows.collect()
}

// Blocks

const BLOCK_COLUMNS: &str = "id, layout_id, name, x, y, state, occupied, max_speed, next_block_id, length, occupancy_locomotive_id, created_at";

fn block_from_row(row: &Row) -> rusqlite::Result<Block> {
    Ok(Block {
        id: row.get("id")?,
        layout_id: row.get("layout_id")?,
        name: row.get("name")?,
        x: row.get("x")?,
        y: row.get("y")?,
        state: row.get("state")?,
        occupied: row.get("occupied")?,
        max_speed: row.get("max_speed")?,
        next_block_id: row.get("next_block_id")?,
        signal_ids: Vec::new(),
        feedback_ids: Vec::new(),
        created_at: row.get("created_at")?,
        length: row.get("length")?,
        occupancy_locomotive_id: row.get("occupancy_locomotive_id")?,
    })
}

fn with_links(conn: &Connection, mut block: Block) -> rusqlite::Result<Block> {
    block.signal_ids = ids(conn, "SELECT signal_id FROM block_signals WHERE block_id = ?1 ORDER BY signal_id", block.id)?;
    block.feedback_ids = ids(conn, "SELECT feedback_id FROM block_feedbacks WHERE block_id = ?1 ORDER BY feedback_id", block.id)?;
    Ok(block)
}

pub fn blocks(conn: &Connection, layout_id: Option<i64>) -> rusqlite::Result<Vec<Block>> {
    let mut statement = conn.prepare(&format!(
        "SELECT {BLOCK_COLUMNS} FROM blocks WHERE (?1 IS NULL OR layout_id = ?1) ORDER BY id"
    ))?;
    let rows: Vec<Block> = statement.query_map([layout_id], block_from_row)?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|block| with_links(conn, block)).collect()
}

pub fn block(conn: &Connection, id: i64) -> rusqlite::Result<Option<Block>> {
    let found = conn
        .query_row(&format!("SELECT {BLOCK_COLUMNS} FROM blocks WHERE id = ?1"), [id], block_from_row)
        .optional()?;
    found.map(|block| with_links(conn, block)).transpose()
}

pub struct NewBlock<'a> {
    pub layout_id: i64,
    pub name: &'a str,
    pub x: f64,
    pub y: f64,
    pub max_speed: i64,
    pub length: Option<f64>,
}

pub fn insert_block(conn: &Connection, block: &NewBlock) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO blocks (layout_id, name, x, y, max_speed, length, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![block.layout_id, block.name, block.x, block.y, block.max_speed, block.length, now()],
    )?;
    let id = conn.last_insert_rowid();
    sync::mark_new(conn, "blocks", id)?;
    Ok(id)
}

/// Write the editable columns back. `occupied` follows the state
pub fn save_block(conn: &Connection, block: &Block) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE blocks SET name = ?2, x = ?3, y = ?4, max_speed = ?5, state = ?6, occupied = ?7, next_block_id = ?8, updated_at = ?9
         WHERE id = ?1",
        params![block.id, block.name, block.x, block.y, block.max_speed, block.state, block.occupied, block.next_block_id, now()],
    )?;
    sync::mark_changed(conn, "blocks", block.id)?;
    Ok(())
}

pub fn delete_block(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    sync::forget(conn, "blocks", id)?;
    conn.execute("DELETE FROM blocks WHERE id = ?1", [id])?;
    Ok(())
}

// Signals

const SIGNAL_COLUMNS: &str = "id, layout_id, name, x, y, signal_type, aspect, created_at";

fn signal_from_row(row: &Row) -> rusqlite::Result<Signal> {
    Ok(Signal {
        id: row.get("id")?,
        layout_id: row.get("layout_id")?,
        name: row.get("name")?,
        x: row.get("x")?,
        y: row.get("y")?,
        signal_type: row.get("signal_type")?,
        aspect: row.get("aspect")?,
        block_ids: Vec::new(),
        created_at: row.get("created_at")?,
    })
}

pub fn signals(conn: &Connection, layout_id: Option<i64>) -> rusqlite::Result<Vec<Signal>> {
    let mut statement = conn.prepare(&format!(
        "SELECT {SIGNAL_COLUMNS} FROM signals WHERE (?1 IS NULL OR layout_id = ?1) ORDER BY id"
    ))?;
    let rows: Vec<Signal> = statement.query_map([layout_id], signal_from_row)?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter()
        .map(|mut signal| {
            signal.block_ids = ids(conn, "SELECT block_id FROM block_signals WHERE signal_id = ?1 ORDER BY block_id", signal.id)?;
            Ok(signal)
        })
        .collect()
}

pub fn signal(conn: &Connection, id: i64) -> rusqlite::Result<Option<Signal>> {
    let found = conn
        .query_row(&format!("SELECT {SIGNAL_COLUMNS} FROM signals WHERE id = ?1"), [id], signal_from_row)
        .optional()?;
    found
        .map(|mut signal| {
            signal.block_ids = ids(conn, "SELECT block_id FROM block_signals WHERE signal_id = ?1 ORDER BY block_id", signal.id)?;
            Ok(signal)
        })
        .transpose()
}

pub fn insert_signal(conn: &Connection, layout_id: i64, name: &str, x: f64, y: f64, signal_type: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO signals (layout_id, name, x, y, signal_type, aspect, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'stop', ?6, ?6)",
        params![layout_id, name, x, y, signal_type, now()],
    )?;
    let id = conn.last_insert_rowid();
    sync::mark_new(conn, "signals", id)?;
    Ok(id)
}

pub fn save_signal(conn: &Connection, signal: &Signal) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE signals SET name = ?2, x = ?3, y = ?4, aspect = ?5, updated_at = ?6 WHERE id = ?1",
        params![signal.id, signal.name, signal.x, signal.y, signal.aspect, now()],
    )?;
    sync::mark_changed(conn, "signals", signal.id)?;
    Ok(())
}

pub fn delete_signal(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    sync::forget(conn, "signals", id)?;
    conn.execute("DELETE FROM signals WHERE id = ?1", [id])?;
    Ok(())
}

// Feedbacks

const FEEDBACK_COLUMNS: &str = "id, layout_id, name, x, y, feedback_number, state, created_at";

fn feedback_from_row(row: &Row) -> rusqlite::Result<Feedback> {
    Ok(Feedback {
        id: row.get("id")?,
        layout_id: row.get("layout_id")?,
        name: row.get("name")?,
        x: row.get("x")?,
        y: row.get("y")?,
        feedback_number: row.get("feedback_number")?,
        state: row.get("state")?,
        block_ids: Vec::new(),
        created_at: row.get("created_at")?,
    })
}

pub fn feedbacks(conn: &Connection, layout_id: Option<i64>) -> rusqlite::Result<Vec<Feedback>> {
    let mut statement = conn.prepare(&format!(
        "SELECT {FEEDBACK_COLUMNS} FROM feedbacks WHERE (?1 IS NULL OR layout_id = ?1) ORDER BY id"
    ))?;
    let rows: Vec<Feedback> = statement.query_map([layout_id], feedback_from_row)?.collect::<rusqlite::Result<_>>()?;
    rows.into_iter()
        .map(|mut feedback| {
            feedback.block_ids = ids(conn, "SELECT block_id FROM block_feedbacks WHERE feedback_id = ?1 ORDER BY block_id", feedback.id)?;
            Ok(feedback)
        })
        .collect()
}

pub fn feedback(conn: &Connection, id: i64) -> rusqlite::Result<Option<Feedback>> {
    let found = conn
        .query_row(&format!("SELECT {FEEDBACK_COLUMNS} FROM feedbacks WHERE id = ?1"), [id], feedback_from_row)
        .optional()?;
    found
        .map(|mut feedback| {
            feedback.block_ids = ids(conn, "SELECT block_id FROM block_feedbacks WHERE feedback_id = ?1 ORDER BY block_id", feedback.id)?;
            Ok(feedback)
        })
        .transpose()
}

pub fn insert_feedback(conn: &Connection, layout_id: i64, name: &str, x: f64, y: f64, number: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO feedbacks (layout_id, name, x, y, feedback_number, state, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)",
        params![layout_id, name, x, y, number, now()],
    )?;
    let id = conn.last_insert_rowid();
    sync::mark_new(conn, "feedbacks", id)?;
    Ok(id)
}

pub fn save_feedback(conn: &Connection, feedback: &Feedback) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE feedbacks SET name = ?2, x = ?3, y = ?4, state = ?5, updated_at = ?6 WHERE id = ?1",
        params![feedback.id, feedback.name, feedback.x, feedback.y, feedback.state, now()],
    )?;
    sync::mark_changed(conn, "feedbacks", feedback.id)?;
    Ok(())
}

pub fn delete_feedback(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    sync::forget(conn, "feedbacks", id)?;
    conn.execute("DELETE FROM feedbacks WHERE id = ?1", [id])?;
    Ok(())
}

// Routes

const ROUTE_COLUMNS: &str = "id, layout_id, name, block_sequence, speed_profile, dwell_times, auto_return,
    enabled, is_running, current_locomotive_id, current_block_index, created_at";

fn route_from_row(row: &Row) -> rusqlite::Result<Route> {
    let sequence: String = row.get("block_sequence")?;
    Ok(Route {
        id: row.get("id")?,
        layout_id: row.get("layout_id")?,
        name: row.get("name")?,
        block_sequence: serde_json::from_str(&sequence).unwrap_or_default(),
        speed_profile: json_column(row, "speed_profile")?,
        dwell_times: json_column(row, "dwell_times")?,
        auto_return: row.get("auto_return")?,
        enabled: row.get("enabled")?,
        is_running: row.get("is_running")?,
        current_block_index: row.get("current_block_index")?,
        created_at: row.get("created_at")?,
        current_locomotive_id: row.get("current_locomotive_id")?,
    })
}

pub fn routes(conn: &Connection, layout_id: Option<i64>) -> rusqlite::Result<Vec<Route>> {
    let mut statement = conn.prepare(&format!(
        "SELECT {ROUTE_COLUMNS} FROM routes WHERE (?1 IS NULL OR layout_id = ?1) ORDER BY id"
    ))?;
    let rows = statement.query_map([layout_id], route_from_row)?;
    rows.collect()
}

pub fn route(conn: &Connection, id: i64) -> rusqlite::Result<Option<Route>> {
    conn.query_row(&format!("SELECT {ROUTE_COLUMNS} FROM routes WHERE id = ?1"), [id], route_from_row)
        .optional()
}

pub struct NewRoute<'a> {
    pub layout_id: i64,
    pub name: &'a str,
    pub block_sequence: &'a [i64],
    pub speed_profile: Option<&'a Value>,
    pub dwell_times: Option<&'a Value>,
    pub auto_return: bool,
}

pub fn insert_route(conn: &Connection, route: &NewRoute) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO routes (layout_id, name, block_sequence, speed_profile, dwell_times, auto_return, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            route.layout_id,
            route.name,
            serde_json::to_string(route.block_sequence).unwrap_or_default(),
            route.speed_profile.map(|value| value.to_string()),
            route.dwell_times.map(|value| value.to_string()),
            route.auto_return,
            now()
        ],
    )?;
    let id = conn.last_insert_rowid();
    sync::mark_new(conn, "routes", id)?;
    Ok(id)
}

pub fn save_route(conn: &Connection, route: &Route) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE routes SET name = ?2, block_sequence = ?3, speed_profile = ?4, enabled = ?5, updated_at = ?6
         WHERE id = ?1",
        params![
            route.id,
            route.name,
            serde_json::to_string(&route.block_sequence).unwrap_or_default(),
            route.speed_profile.as_ref().map(|value| value.to_string()),
            route.enabled,
            now()
        ],
    )?;
    sync::mark_changed(conn, "routes", route.id)?;
    Ok(())
}

pub fn delete_route(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    sync::forget(conn, "routes", id)?;
    conn.execute("DELETE FROM routes WHERE id = ?1", [id])?;
    Ok(())
}

/// Replace the feedbacks a block reads its occupancy from
pub fn set_block_feedbacks(conn: &Connection, block_id: i64, feedback_ids: &[i64]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM block_feedbacks WHERE block_id = ?1", [block_id])?;
    for feedback_id in feedback_ids {
        conn.execute(
            "INSERT INTO block_feedbacks (block_id, feedback_id) VALUES (?1, ?2)",
            params![block_id, feedback_id],
        )?;
    }
    sync::mark_changed(conn, "blocks", block_id)?;
    Ok(())
}

/// Replace the signals that show the aspect of a block
pub fn set_block_signals(conn: &Connection, block_id: i64, signal_ids: &[i64]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM block_signals WHERE block_id = ?1", [block_id])?;
    for signal_id in signal_ids {
        conn.execute("INSERT INTO block_signals (block_id, signal_id) VALUES (?1, ?2)", params![block_id, signal_id])?;
    }
    sync::mark_changed(conn, "blocks", block_id)?;
    Ok(())
}

/// Replace the blocks a signal protects
pub fn set_signal_blocks(conn: &Connection, signal_id: i64, block_ids: &[i64]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM block_signals WHERE signal_id = ?1", [signal_id])?;
    for block_id in block_ids {
        conn.execute("INSERT INTO block_signals (block_id, signal_id) VALUES (?1, ?2)", params![block_id, signal_id])?;
    }
    sync::mark_changed(conn, "signals", signal_id)?;
    Ok(())
}

/// Add the columns that newer versions need to a database made before them
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(blocks)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    if !columns.iter().any(|column| column == "next_block_id") {
        conn.execute_batch("ALTER TABLE blocks ADD COLUMN next_block_id INTEGER REFERENCES blocks(id);")?;
    }
    Ok(())
}
