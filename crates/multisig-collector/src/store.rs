// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Leon Frenzel

//! SQLite-backed store. This task only opens the database (health +
//! scaffold); proposal/partial and party-roster tables land in Tasks 5–6.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::Connection;

/// Wraps a single `rusqlite::Connection` behind a mutex — `Connection` is
/// `Send` but not `Sync`, and axum handlers need a `Sync` shared state.
/// Mirrors `chain-gateway-core::db::EventDb`'s pattern.
pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Opens (creating if absent) the SQLite database at `path`, enabling
    /// WAL mode for concurrent reader/writer access.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create database directory {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open sqlite database {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .context("set WAL journal mode")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// In-memory store for tests — no file on disk.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory sqlite database")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Confirms the underlying connection is alive and can run a query —
    /// used by the health handler as a lightweight readiness check.
    pub fn is_alive(&self) -> bool {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.execute_batch("SELECT 1;").is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_store_opens_and_is_alive() {
        let store = Store::open_in_memory().expect("open in-memory store");
        assert!(store.is_alive());
    }

    #[test]
    fn file_store_opens_creating_parent_dirs() {
        let dir = tempfile_dir();
        let path = dir.join("nested").join("collector.sqlite");
        let store = Store::open(&path).expect("open file-backed store");
        assert!(store.is_alive());
        assert!(path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "multisig-collector-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        dir
    }
}
