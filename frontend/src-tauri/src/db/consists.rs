//! Consists: named groups of locomotives that run together. The first member is the lead. A locomotive is in at most
//! one consist, so a drive command never reaches the same locomotive twice. Each consist has a DCC consist address,
//! which the decoders of its members follow once their CV19 is set (plan Phase 10, item 31)

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;

use super::now;

/// The lowest and highest DCC short address a consist can use
pub const ADDRESS_MIN: i64 = 1;
pub const ADDRESS_MAX: i64 = 127;

#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub locomotive_id: i64,
    pub name: String,
    pub dcc_address: i64,
    /// True when this locomotive runs the opposite way to the consist
    pub reversed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Consist {
    pub id: i64,
    pub name: String,
    pub address: i64,
    pub members: Vec<Member>,
}

/// A member as given to the API: which locomotive, and whether it runs reversed
pub type MemberInput = (i64, bool);

/// Add the consist address to a database made before it existed. The default is 1, the address LocoLink starts with
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(consists)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    if !columns.iter().any(|column| column == "address") {
        conn.execute_batch("ALTER TABLE consists ADD COLUMN address INTEGER NOT NULL DEFAULT 1;")?;
    }
    Ok(())
}

fn members_of(conn: &Connection, consist_id: i64) -> rusqlite::Result<Vec<Member>> {
    let mut statement = conn.prepare(
        "SELECT l.id, l.name, l.dcc_address, m.reversed FROM consist_members m
         JOIN locomotives l ON l.id = m.locomotive_id
         WHERE m.consist_id = ?1 ORDER BY m.position",
    )?;
    let rows = statement.query_map([consist_id], |row| {
        Ok(Member {
            locomotive_id: row.get(0)?,
            name: row.get(1)?,
            dcc_address: row.get(2)?,
            reversed: row.get::<_, i64>(3)? != 0,
        })
    })?;
    rows.collect()
}

fn load(conn: &Connection, id: i64, name: String, address: i64) -> rusqlite::Result<Consist> {
    Ok(Consist { id, name, address, members: members_of(conn, id)? })
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Consist>> {
    let mut statement = conn.prepare("SELECT id, name, address FROM consists ORDER BY name")?;
    let rows: Vec<(i64, String, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|(id, name, address)| load(conn, id, name, address)).collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Consist>> {
    let row: Option<(String, i64)> = conn
        .query_row("SELECT name, address FROM consists WHERE id = ?1", [id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    row.map(|(name, address)| load(conn, id, name, address)).transpose()
}

/// The lowest consist address no consist uses yet, or None if all of them are taken
pub fn free_address(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    let mut statement = conn.prepare("SELECT address FROM consists")?;
    let used: Vec<i64> = statement.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok((ADDRESS_MIN..=ADDRESS_MAX).find(|address| !used.contains(address)))
}

/// True if another consist already uses this address
pub fn address_taken(conn: &Connection, address: i64, except_id: Option<i64>) -> rusqlite::Result<bool> {
    let owner: Option<i64> = conn
        .query_row("SELECT id FROM consists WHERE address = ?1", [address], |row| row.get(0))
        .optional()?;
    Ok(matches!(owner, Some(id) if Some(id) != except_id))
}

/// Write the members in order, replacing any earlier ones
fn write_members(tx: &Transaction, consist_id: i64, members: &[MemberInput]) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM consist_members WHERE consist_id = ?1", [consist_id])?;
    for (position, (locomotive_id, reversed)) in members.iter().enumerate() {
        tx.execute(
            "INSERT INTO consist_members (consist_id, locomotive_id, position, reversed) VALUES (?1, ?2, ?3, ?4)",
            params![consist_id, locomotive_id, position as i64, *reversed as i64],
        )?;
    }
    Ok(())
}

pub fn insert(conn: &mut Connection, name: &str, address: i64, members: &[MemberInput]) -> rusqlite::Result<i64> {
    let tx = conn.transaction()?;
    let stamp = now();
    tx.execute(
        "INSERT INTO consists (name, address, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![name, address, stamp],
    )?;
    let id = tx.last_insert_rowid();
    write_members(&tx, id, members)?;
    tx.commit()?;
    Ok(id)
}

/// Change the name, the address and, when given, the members. Returns false if the consist does not exist
pub fn update(
    conn: &mut Connection,
    id: i64,
    name: Option<&str>,
    address: Option<i64>,
    members: Option<&[MemberInput]>,
) -> rusqlite::Result<bool> {
    let tx = conn.transaction()?;
    let exists: i64 = tx.query_row("SELECT COUNT(*) FROM consists WHERE id = ?1", [id], |row| row.get(0))?;
    if exists == 0 {
        return Ok(false);
    }
    if let Some(name) = name {
        tx.execute("UPDATE consists SET name = ?2, updated_at = ?3 WHERE id = ?1", params![id, name, now()])?;
    }
    if let Some(address) = address {
        tx.execute("UPDATE consists SET address = ?2, updated_at = ?3 WHERE id = ?1", params![id, address, now()])?;
    }
    if let Some(members) = members {
        write_members(&tx, id, members)?;
    }
    tx.commit()?;
    Ok(true)
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute("DELETE FROM consists WHERE id = ?1", [id])? > 0)
}

/// The consist a locomotive belongs to, if any
pub fn consist_of(conn: &Connection, locomotive_id: i64) -> rusqlite::Result<Option<i64>> {
    conn.query_row("SELECT consist_id FROM consist_members WHERE locomotive_id = ?1", [locomotive_id], |row| row.get(0))
        .optional()
}
