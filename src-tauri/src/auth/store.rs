//! Where the session is kept: the operating system's keychain in the app, and memory in tests

use std::sync::Mutex;

use super::Session;

const SERVICE: &str = "com.trainlink.app";
const ACCOUNT: &str = "supabase-session";

pub trait SessionStore: Send + Sync {
    fn load(&self) -> Option<Session>;
    fn save(&self, session: &Session) -> Result<(), String>;
    fn clear(&self);
}

/// The macOS Keychain (or Windows Credential Manager, once built there)
pub struct KeychainStore;

impl KeychainStore {
    pub fn new() -> KeychainStore {
        KeychainStore
    }

    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|error| error.to_string())
    }
}

impl SessionStore for KeychainStore {
    fn load(&self) -> Option<Session> {
        let text = Self::entry().ok()?.get_password().ok()?;
        serde_json::from_str(&text).ok()
    }

    fn save(&self, session: &Session) -> Result<(), String> {
        let text = serde_json::to_string(session).map_err(|error| error.to_string())?;
        Self::entry()?.set_password(&text).map_err(|error| error.to_string())
    }

    fn clear(&self) {
        if let Ok(entry) = Self::entry() {
            let _ = entry.delete_credential();
        }
    }
}

/// Keeps the session in memory. For tests
pub struct MemoryStore {
    session: Mutex<Option<Session>>,
}

impl MemoryStore {
    pub fn new() -> MemoryStore {
        MemoryStore { session: Mutex::new(None) }
    }
}

impl SessionStore for MemoryStore {
    fn load(&self) -> Option<Session> {
        self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    fn save(&self, session: &Session) -> Result<(), String> {
        *self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(session.clone());
        Ok(())
    }

    fn clear(&self) {
        *self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}
