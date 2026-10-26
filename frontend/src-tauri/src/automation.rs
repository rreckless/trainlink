//! Automatic driving and collision avoidance. A route run moves its locomotive through the route's blocks in order, or back
//! again with auto-return. The locomotive takes the speed of each block from the route's speed profile, or the block's
//! maximum speed, and waits at each block for its dwell time.
//!
//! Collision avoidance: a run holds a block it is in and claims the block it is about to enter, so no other run can take
//! it. It only claims a block that is free of trains, and it only moves into a block it has claimed. A run that cannot
//! claim its next block stops and waits for it. A cycle of runs waiting on each other is broken by stopping the newest
//! run in the cycle. Signals show stop when a block is occupied, and runs never enter an occupied block, so the signals
//! and the runs agree.

use std::collections::{HashMap, HashSet};
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
/// How many notices are kept
const NOTICES_KEPT: usize = 20;

/// What a running route remembers that is not stored: its direction, when it may leave a block, the speed last sent,
/// and the block it is waiting to claim
#[derive(Debug, Clone)]
pub struct Run {
    pub forward: bool,
    pub leave_at: Option<Instant>,
    pub sent: Option<(i64, bool)>,
    pub waiting_for: Option<i64>,
}

fn new_run() -> Run {
    Run { forward: true, leave_at: None, sent: None, waiting_for: None }
}

fn notice(inner: &mut Inner, text: String) {
    inner.notices.push(text);
    if inner.notices.len() > NOTICES_KEPT {
        let excess = inner.notices.len() - NOTICES_KEPT;
        inner.notices.drain(..excess);
    }
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
pub fn occupied_map(inner: &Inner) -> rusqlite::Result<HashMap<i64, bool>> {
    let detected = occupancy::detected(&inner.db, &inner.z21.feedback)?;
    let mut map = HashMap::new();
    for block in trackside::blocks(&inner.db, None)? {
        map.insert(block.id, detected.get(&block.id).copied().unwrap_or(block.occupied));
    }
    Ok(map)
}

/// The block a run is heading to next, in its direction, if the route has one there
fn next_index(route: &trackside::Route, index: usize, forward: bool) -> Option<usize> {
    if forward {
        (index + 1 < route.block_sequence.len()).then_some(index + 1)
    } else {
        index.checked_sub(1)
    }
}

/// Send a speed and direction to a locomotive, and note it as moving for the moving-time record
fn send_drive(inner: &mut Inner, address: i64, speed: i64, forward: bool, now: Instant) {
    if let Ok(frame) = commands::loco_drive(address, speed, forward) {
        inner.z21.send(&frame);
    }
    let Inner { db, moving, .. } = inner;
    moving.observe(db, address, Some(speed), now);
}

/// Stop a run: its route is no longer running, its blocks are released, and its locomotive is stopped
fn finish(inner: &mut Inner, route_id: i64, address: Option<i64>, now: Instant) -> rusqlite::Result<()> {
    inner.runs.remove(&route_id);
    inner.claims.retain(|_, holder| *holder != route_id);
    inner.db.execute("UPDATE routes SET is_running = 0, updated_at = ?2 WHERE id = ?1", rusqlite::params![route_id, crate::db::now()])?;
    if let Some(address) = address {
        send_drive(inner, address, 0, true, now);
    }
    Ok(())
}

/// Start a route with a locomotive. The locomotive is in the route's first block, which must not be held by another run
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
    let first = route.block_sequence[0];
    if let Some(holder) = inner.claims.get(&first).copied().filter(|holder| *holder != route_id) {
        let name = trackside::route(&inner.db, holder)?.map(|other| other.name).unwrap_or_default();
        return Err(ApiError::conflict(format!("The first block is in use by the run {name}")));
    }
    inner.db.execute(
        "UPDATE routes SET is_running = 1, current_block_index = 0, current_locomotive_id = ?2, updated_at = ?3 WHERE id = ?1",
        rusqlite::params![route_id, locomotive_id, crate::db::now()],
    )?;
    inner.claims.insert(first, route_id);
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

/// The route that a waiting run is waiting on, and the run it waits for, if another run holds that block
fn waiting_on(inner: &Inner) -> HashMap<i64, i64> {
    let mut edges = HashMap::new();
    for (route_id, run) in &inner.runs {
        if let Some(block) = run.waiting_for {
            if let Some(holder) = inner.claims.get(&block).filter(|holder| *holder != route_id) {
                edges.insert(*route_id, *holder);
            }
        }
    }
    edges
}

/// Break any cycle of runs waiting on each other by stopping the newest run in it. Returns the stopped routes
fn break_cycles(inner: &mut Inner, now: Instant) -> rusqlite::Result<Vec<i64>> {
    let edges = waiting_on(inner);
    let mut stopped = Vec::new();
    for start in edges.keys().copied().collect::<Vec<_>>() {
        let mut path = vec![start];
        let mut current = start;
        while let Some(next) = edges.get(&current).copied() {
            if let Some(position) = path.iter().position(|id| *id == next) {
                let cycle = path[position..].to_vec();
                let victim = *cycle.iter().max().unwrap_or(&next);
                if !stopped.contains(&victim) && inner.runs.contains_key(&victim) {
                    let name = trackside::route(&inner.db, victim)?.map(|route| route.name).unwrap_or_default();
                    let address = match trackside::route(&inner.db, victim)?.and_then(|route| route.current_locomotive_id) {
                        Some(loco) => locomotives::get(&inner.db, loco)?.map(|loco| loco.dcc_address),
                        None => None,
                    };
                    finish(inner, victim, address, now)?;
                    notice(inner, format!("Stopped {name}: it and another run were each waiting for a block the other held"));
                    stopped.push(victim);
                }
                break;
            }
            path.push(next);
            current = next;
        }
    }
    Ok(stopped)
}

/// Raise an alarm for each block that has become occupied with no run holding it, and clear the alarms of blocks that
/// are free again. A block that was already occupied at the last check does not raise a new alarm: a train a run has just
/// left in its last block is not unexpected
fn check_unexpected(inner: &mut Inner, occupied: &HashMap<i64, bool>) -> rusqlite::Result<()> {
    let now_occupied: HashSet<i64> = occupied.iter().filter(|(_, is)| **is).map(|(block, _)| *block).collect();
    let free: Vec<i64> = inner.alarms.keys().filter(|block| !now_occupied.contains(block)).copied().collect();
    for block in free {
        inner.alarms.remove(&block);
    }
    for block in &now_occupied {
        if inner.seen_occupied.contains(block) || inner.claims.contains_key(block) || inner.alarms.contains_key(block) {
            continue;
        }
        let name = trackside::block(&inner.db, *block)?.map(|found| found.name).unwrap_or_else(|| format!("block {block}"));
        inner.alarms.insert(*block, format!("Unexpected train in {name}: no run has claimed it"));
        notice(inner, format!("Unexpected train in {name}"));
    }
    inner.seen_occupied = now_occupied;
    Ok(())
}

/// Dismiss the alarm for a block. The block is still held, so runs keep waiting for it until it is free
pub fn dismiss_alarm(inner: &mut Inner, block: i64) -> bool {
    inner.alarms.remove(&block).is_some()
}

/// One check of every running route. Routes that are running but were not started in this session are stopped, since
/// their run was lost when the app closed
pub fn tick(inner: &mut Inner, now: Instant) -> rusqlite::Result<()> {
    if inner.z21.track_state() != "ready" {
        return Ok(());
    }
    let occupied = occupied_map(inner)?;
    check_unexpected(inner, &occupied)?;
    let running: Vec<trackside::Route> = trackside::routes(&inner.db, None)?.into_iter().filter(|route| route.is_running).collect();
    for route in running {
        if !inner.runs.contains_key(&route.id) {
            let address = match route.current_locomotive_id {
                Some(loco) => locomotives::get(&inner.db, loco)?.map(|loco| loco.dcc_address),
                None => None,
            };
            finish(inner, route.id, address, now)?;
            notice(inner, format!("Stopped {}: it was still running when the app started", route.name));
        }
    }
    break_cycles(inner, now)?;
    let running: Vec<trackside::Route> = trackside::routes(&inner.db, None)?.into_iter().filter(|route| route.is_running).collect();
    for route in running {
        let occupied = occupied_map(inner)?;
        advance(inner, &route, &occupied, now)?;
    }
    Ok(())
}

/// Claim the block ahead of a run, in its direction. True if the run holds it now, or the route has no block ahead.
/// A block held by another run, or with a train in it, cannot be claimed: the run then waits for it
fn claim_ahead(inner: &mut Inner, route: &trackside::Route, run: &mut Run, index: usize, occupied: &HashMap<i64, bool>) -> bool {
    let Some(next) = next_index(route, index, run.forward) else {
        run.waiting_for = None;
        return true;
    };
    let block = route.block_sequence[next];
    if inner.claims.get(&block) == Some(&route.id) {
        run.waiting_for = None;
        return true;
    }
    let held_elsewhere = inner.claims.get(&block).is_some_and(|holder| *holder != route.id);
    if held_elsewhere || occupied.get(&block).copied().unwrap_or(false) {
        run.waiting_for = Some(block);
        return false;
    }
    inner.claims.insert(block, route.id);
    run.waiting_for = None;
    true
}

/// Move one running route on as far as the trains on the layout allow
fn advance(inner: &mut Inner, route: &trackside::Route, occupied: &HashMap<i64, bool>, now: Instant) -> rusqlite::Result<()> {
    let Some(loco_id) = route.current_locomotive_id else { return finish(inner, route.id, None, now) };
    let Some(loco) = locomotives::get(&inner.db, loco_id)? else { return finish(inner, route.id, None, now) };
    let address = loco.dcc_address;
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

    // The train has entered the next block. Only a block this run has claimed can be entered, so an occupied block with
    // no claim is not taken as this train
    if let Some(next) = next_index(route, index, run.forward) {
        let block = route.block_sequence[next];
        if inner.claims.get(&block) == Some(&route.id) && occupied.get(&block).copied().unwrap_or(false) {
            // The block behind is released: the train has left it
            let behind = route.block_sequence[index];
            if inner.claims.get(&behind) == Some(&route.id) {
                inner.claims.remove(&behind);
            }
            index = next;
            inner.db.execute(
                "UPDATE routes SET current_block_index = ?2, updated_at = ?3 WHERE id = ?1",
                rusqlite::params![route.id, index as i64, crate::db::now()],
            )?;
            // Hold the block ahead before the dwell, so no other run can take it while this one waits
            claim_ahead(inner, route, &mut run, index, occupied);
            let dwell = dwell_for(route, index);
            if !dwell.is_zero() {
                run.leave_at = Some(now + dwell);
                run.sent = Some((0, run.forward));
                send_drive(inner, address, 0, run.forward, now);
                inner.runs.insert(route.id, run);
                return Ok(());
            }
        }
    }

    // At the end of the route in this direction
    let at_end = next_index(route, index, run.forward).is_none();
    if at_end {
        if route.auto_return {
            run.forward = !run.forward;
            // The run goes back the way it came, so it claims the block ahead in its new direction now
            claim_ahead(inner, route, &mut run, index, occupied);
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

    // Claim the block ahead. If it cannot be claimed, the train holds here
    if !claim_ahead(inner, route, &mut run, index, occupied) {
        if run.sent != Some((0, run.forward)) {
            send_drive(inner, address, 0, run.forward, now);
            run.sent = Some((0, run.forward));
        }
        inner.runs.insert(route.id, run);
        return Ok(());
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
