//! Automatic driving. A route run moves its locomotive through the route's blocks in order, or back again with
//! auto-return. The locomotive takes the speed of each block from the route's speed profile, or the block's maximum
//! speed. It waits at each block for the block's dwell time. It moves on when the next block becomes occupied, which
//! is when the train has entered it. At the end of the route it stops, or turns round with auto-return.
//! Other trains are not checked here: collision avoidance is a later item in the plan.

use std::sync::Arc;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use serde_json::Value;

use crate::db::{locomotives, trackside};
use crate::http::ApiError;
use crate::occupancy;
use crate::state::{AppState, Inner};
use crate::z21::commands;

/// How often running routes are checked
pub const TICK: Duration = Duration::from_millis(500);

/// What a running route remembers that is not stored: its direction, when it may leave a block, and the speed last sent
#[derive(Debug, Clone)]
pub struct Run {
    pub forward: bool,
    pub leave_at: Option<Instant>,
    pub sent: Option<(i64, bool)>,
}

fn new_run() -> Run {
    Run { forward: true, leave_at: None, sent: None }
}

fn dwell_for(route: &trackside::Route, index: usize) -> Duration {
    route
        .dwell_times
        .as_ref()
        .and_then(|times| times.get(index))
        .and_then(Value::as_f64)
        .filter(|seconds| *seconds > 0.0)
        .map(Duration::from_secs_f64)
        .unwrap_or_default()
}

/// The speed for a block: the route's profile where it sets one, otherwise the block's maximum speed
fn speed_for(conn: &Connection, route: &trackside::Route, index: usize) -> rusqlite::Result<i64> {
    if let Some(speed) = route.speed_profile.as_ref().and_then(|profile| profile.get(index)).and_then(Value::as_i64) {
        return Ok(speed);
    }
    conn.query_row("SELECT max_speed FROM blocks WHERE id = ?1", [route.block_sequence[index]], |row| row.get(0))
}

/// Whether each block is occupied now, by its feedback where that has reported, otherwise by its own state
fn occupied_map(inner: &Inner) -> rusqlite::Result<std::collections::HashMap<i64, bool>> {
    let detected = occupancy::detected(&inner.db, &inner.z21.feedback)?;
    let mut map = std::collections::HashMap::new();
    for block in trackside::blocks(&inner.db, None)? {
        map.insert(block.id, detected.get(&block.id).copied().unwrap_or(block.occupied));
    }
    Ok(map)
}

/// Send a speed and direction to a locomotive, and note it as moving for the moving-time record
fn send_drive(inner: &mut Inner, address: i64, speed: i64, forward: bool, now: Instant) {
    if let Ok(frame) = commands::loco_drive(address, speed, forward) {
        inner.z21.send(&frame);
    }
    let Inner { db, moving, .. } = inner;
    moving.observe(db, address, Some(speed), now);
}

/// Stop a run: its route is no longer running, and its locomotive is stopped
fn finish(inner: &mut Inner, route_id: i64, address: Option<i64>, now: Instant) -> rusqlite::Result<()> {
    inner.runs.remove(&route_id);
    inner.db.execute("UPDATE routes SET is_running = 0, updated_at = ?2 WHERE id = ?1", rusqlite::params![route_id, crate::db::now()])?;
    if let Some(address) = address {
        send_drive(inner, address, 0, true, now);
    }
    Ok(())
}

/// Start a route with a locomotive. The locomotive is in the route's first block, and is driven from there
pub fn start(inner: &mut Inner, route_id: i64, locomotive_id: i64, now: Instant) -> Result<(), ApiError> {
    let route = trackside::route(&inner.db, route_id)?.ok_or_else(|| ApiError::not_found("Route", route_id))?;
    if !route.enabled {
        return Err(ApiError::conflict("The route is disabled"));
    }
    if route.is_running {
        return Err(ApiError::conflict("The route is already running"));
    }
    if locomotives::get(&inner.db, locomotive_id)?.is_none() {
        return Err(ApiError::not_found("Locomotive", locomotive_id));
    }
    inner.db.execute(
        "UPDATE routes SET is_running = 1, current_block_index = 0, current_locomotive_id = ?2, updated_at = ?3 WHERE id = ?1",
        rusqlite::params![route_id, locomotive_id, crate::db::now()],
    )?;
    inner.runs.insert(route_id, new_run());
    let route = trackside::route(&inner.db, route_id)?.ok_or_else(|| ApiError::internal("Route was not saved"))?;
    let occupied = occupied_map(inner)?;
    advance(inner, &route, &occupied, now)?;
    Ok(())
}

/// Stop a route that is running, and its locomotive with it
pub fn stop(inner: &mut Inner, route_id: i64, now: Instant) -> Result<(), ApiError> {
    let route = trackside::route(&inner.db, route_id)?.ok_or_else(|| ApiError::not_found("Route", route_id))?;
    if !route.is_running {
        return Err(ApiError::conflict("The route is not running"));
    }
    let address = match route.current_locomotive_id {
        Some(loco) => locomotives::get(&inner.db, loco)?.map(|loco| loco.dcc_address),
        None => None,
    };
    finish(inner, route_id, address, now)?;
    Ok(())
}

/// One check of every running route. Routes that are running but were not started in this session are stopped, since
/// their run was lost when the app closed
pub fn tick(inner: &mut Inner, now: Instant) -> rusqlite::Result<()> {
    if inner.z21.track_state() != "ready" {
        return Ok(());
    }
    let running: Vec<trackside::Route> = trackside::routes(&inner.db, None)?.into_iter().filter(|route| route.is_running).collect();
    for route in running {
        if !inner.runs.contains_key(&route.id) {
            let address = match route.current_locomotive_id {
                Some(loco) => locomotives::get(&inner.db, loco)?.map(|loco| loco.dcc_address),
                None => None,
            };
            finish(inner, route.id, address, now)?;
            continue;
        }
        let occupied = occupied_map(inner)?;
        advance(inner, &route, &occupied, now)?;
    }
    Ok(())
}

/// Move one running route on as far as the trains on the layout allow
fn advance(inner: &mut Inner, route: &trackside::Route, occupied: &std::collections::HashMap<i64, bool>, now: Instant) -> rusqlite::Result<()> {
    let Some(loco_id) = route.current_locomotive_id else { return finish(inner, route.id, None, now) };
    let Some(loco) = locomotives::get(&inner.db, loco_id)? else { return finish(inner, route.id, None, now) };
    let address = loco.dcc_address;
    let len = route.block_sequence.len();
    let mut run = inner.runs.get(&route.id).cloned().unwrap_or_else(new_run);
    let mut index = route.current_block_index.max(0) as usize;

    // Waiting at a block for its dwell time
    if let Some(at) = run.leave_at {
        if now < at {
            inner.runs.insert(route.id, run);
            return Ok(());
        }
        run.leave_at = None;
    }

    // The train has entered the next block, so the run moves on into it
    let next = if run.forward { index + 1 } else { index.wrapping_sub(1) };
    if next < len && occupied.get(&route.block_sequence[next]).copied().unwrap_or(false) {
        index = next;
        inner.db.execute(
            "UPDATE routes SET current_block_index = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![route.id, index as i64, crate::db::now()],
        )?;
        let dwell = dwell_for(route, index);
        if !dwell.is_zero() {
            run.leave_at = Some(now + dwell);
            run.sent = Some((0, run.forward));
            send_drive(inner, address, 0, run.forward, now);
            inner.runs.insert(route.id, run);
            return Ok(());
        }
    }

    // At the end of the route in this direction
    let at_end = if run.forward { index + 1 == len } else { index == 0 };
    if at_end {
        if route.auto_return {
            run.forward = !run.forward;
            let dwell = dwell_for(route, index);
            if !dwell.is_zero() {
                run.leave_at = Some(now + dwell);
            }
            run.sent = Some((0, run.forward));
            send_drive(inner, address, 0, run.forward, now);
            inner.runs.insert(route.id, run);
            return Ok(());
        }
        return finish(inner, route.id, Some(address), now);
    }

    // Drive this block's speed, in this direction
    let speed = speed_for(&inner.db, route, index)?;
    if run.sent != Some((speed, run.forward)) {
        send_drive(inner, address, speed, run.forward, now);
        run.sent = Some((speed, run.forward));
    }
    inner.runs.insert(route.id, run);
    Ok(())
}

/// Check the running routes every TICK, for as long as the app runs
pub fn spawn_loop(app: Arc<AppState>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(TICK);
        let mut inner = app.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = tick(&mut inner, Instant::now());
    });
}
