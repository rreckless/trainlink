//! The application state shared by the API, the Z21 link, and the moving time tracker

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::moving_time::MovingTime;
use crate::z21::Z21;

/// Everything that changes at runtime, behind one lock so the Z21 reader and the API never disagree
pub struct Inner {
    pub db: Connection,
    pub z21: Z21,
    pub moving: MovingTime,
}

pub struct AppState {
    pub inner: Mutex<Inner>,
    /// Locomotive pictures, one file per locomotive
    pub images: PathBuf,
}

impl AppState {
    pub fn new(db: Connection, images: PathBuf) -> AppState {
        AppState {
            inner: Mutex::new(Inner { db, z21: Z21::new(), moving: MovingTime::new() }),
            images,
        }
    }
}
