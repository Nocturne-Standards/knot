// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Leon Frenzel

//! SQLite-backed store: health scaffold (Task 4) plus the `proposals` table
//! (Task 5). Party-roster tables land in Task 6.
//!
//! `proposals` holds one row per content-addressed proposal id. Partials
//! are **not** a separate table — per the task brief we keep the whole
//! `ProposalDto` (intent + signed_digest + threshold + partials) as a single
//! `body_json` column, rewritten atomically (single `UPDATE`, under the same
//! connection mutex) on every partial append. This makes "append only,
//! never mutate `signed_digest`" trivial to guarantee: the append path
//! deserializes the stored blob, pushes one partial, and re-serializes —
//! there is no code path that touches `signed_digest` or `intent`.

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use crate::dto::{PartialDto, ProposalDto, ProposalSummary};

/// Wraps a single `rusqlite::Connection` behind a mutex — `Connection` is
/// `Send` but not `Sync`, and axum handlers need a `Sync` shared state.
/// Mirrors `chain-gateway-core::db::EventDb`'s pattern.
pub struct Store {
    conn: Mutex<Connection>,
}

/// Result of `Store::create_proposal`.
pub enum CreateOutcome {
    /// New row inserted.
    Created,
    /// An identical (content-addressed) proposal already existed — create
    /// is idempotent for byte-identical bodies.
    AlreadyExists,
    /// A row already exists under this id with a **different** body. Since
    /// `id` = hash of `signed_digest`, this can only happen on a hash
    /// collision or a client bug (e.g. resubmitting with mutated intent
    /// fields under a stale digest) — never treated as a normal path.
    Conflict,
}

/// Result of `Store::append_partial`.
pub enum AppendOutcome {
    /// Partial appended; carries the full updated blob.
    Appended(ProposalDto),
    /// No proposal exists under this id.
    NotFound,
    /// `signer_pk` already has a partial on this proposal.
    DuplicatePk,
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
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// In-memory store for tests — no file on disk.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory sqlite database")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS proposals (
                id TEXT PRIMARY KEY,
                digest TEXT NOT NULL,
                body_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );",
        )
        .context("create proposals table")?;
        Ok(())
    }

    /// Confirms the underlying connection is alive and can run a query —
    /// used by the health handler as a lightweight readiness check.
    pub fn is_alive(&self) -> bool {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.execute_batch("SELECT 1;").is_ok()
    }

    /// Inserts a new proposal under `id` (caller-computed content-addressed
    /// id, see `dto::digest_to_id`). `digest` is the normalized
    /// `"0x"+lowercase-hex` digest string, stored redundantly for cheap
    /// `SELECT digest` listing without a full JSON parse.
    pub fn create_proposal(&self, id: &str, digest: &str, dto: &ProposalDto) -> Result<CreateOutcome> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let existing: Option<String> = conn
            .query_row(
                "SELECT body_json FROM proposals WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .context("query existing proposal")?;
        if let Some(existing_body) = existing {
            let existing_dto: ProposalDto =
                serde_json::from_str(&existing_body).context("parse stored proposal body")?;
            return Ok(if &existing_dto == dto {
                CreateOutcome::AlreadyExists
            } else {
                CreateOutcome::Conflict
            });
        }
        let body_json = serde_json::to_string(dto).context("serialize proposal body")?;
        let created_at = now_unix();
        conn.execute(
            "INSERT INTO proposals (id, digest, body_json, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, digest, body_json, created_at],
        )
        .context("insert proposal")?;
        Ok(CreateOutcome::Created)
    }

    /// Full blob for `GET /v1/proposals/:id`. `None` if `id` is unknown.
    pub fn get_proposal(&self, id: &str) -> Result<Option<ProposalDto>> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let body: Option<String> = conn
            .query_row(
                "SELECT body_json FROM proposals WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .context("query proposal")?;
        body.map(|b| serde_json::from_str(&b).context("parse stored proposal body"))
            .transpose()
    }

    /// Summaries for `GET /v1/proposals`, most recently created first.
    pub fn list_proposals(&self) -> Result<Vec<ProposalSummary>> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT id, digest, body_json, created_at FROM proposals ORDER BY created_at DESC, id ASC")
            .context("prepare list query")?;
        let mut rows = stmt.query([]).context("run list query")?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().context("step list query")? {
            let id: String = row.get(0)?;
            let digest: String = row.get(1)?;
            let body_json: String = row.get(2)?;
            let created_at: i64 = row.get(3)?;
            let dto: ProposalDto =
                serde_json::from_str(&body_json).context("parse stored proposal body")?;
            out.push(ProposalSummary {
                id,
                signed_digest: digest,
                threshold: dto.threshold,
                partials_count: dto.partials.len(),
                created_at,
            });
        }
        Ok(out)
    }

    /// Appends one partial to the proposal's `partials` list — read,
    /// dedup-check, push, rewrite `body_json`, all under the same lock so
    /// two concurrent appends can't race and lose one.
    pub fn append_partial(&self, id: &str, partial: PartialDto) -> Result<AppendOutcome> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let body: Option<String> = conn
            .query_row(
                "SELECT body_json FROM proposals WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .context("query proposal for append")?;
        let Some(body_json) = body else {
            return Ok(AppendOutcome::NotFound);
        };
        let mut dto: ProposalDto =
            serde_json::from_str(&body_json).context("parse stored proposal body")?;

        let new_pk_norm = partial.signer_pk.trim_start_matches("0x").to_ascii_lowercase();
        let duplicate = dto.partials.iter().any(|p| {
            p.signer_pk.trim_start_matches("0x").to_ascii_lowercase() == new_pk_norm
        });
        if duplicate {
            return Ok(AppendOutcome::DuplicatePk);
        }

        let digest_before = dto.signed_digest.clone();
        dto.partials.push(partial);
        debug_assert_eq!(
            dto.signed_digest, digest_before,
            "append_partial must never touch signed_digest"
        );

        let new_body_json = serde_json::to_string(&dto).context("serialize updated proposal body")?;
        conn.execute(
            "UPDATE proposals SET body_json = ?1 WHERE id = ?2",
            params![new_body_json, id],
        )
        .context("update proposal body")?;
        Ok(AppendOutcome::Appended(dto))
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::IntentDto;

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

    fn sample_dto(digest: &str) -> ProposalDto {
        ProposalDto {
            version: 1,
            intent: IntentDto {
                chain_id: 1,
                committee_id: 7,
                nonce: 3,
                target_contract_id: "0x11".to_string(),
                function_name: "set_service".to_string(),
                call_args: "0x0001".to_string(),
                deadline: 1000,
                human_summary: Some("hint".to_string()),
            },
            signed_digest: digest.to_string(),
            threshold: 2,
            partials: Vec::new(),
        }
    }

    #[test]
    fn create_get_append_list_round_trip() {
        let store = Store::open_in_memory().expect("open store");
        let digest = "0x".to_string() + &"ab".repeat(32);
        let id = digest.trim_start_matches("0x").to_string();
        let dto = sample_dto(&digest);

        matches!(
            store.create_proposal(&id, &digest, &dto).unwrap(),
            CreateOutcome::Created
        )
        .then_some(())
        .expect("expected Created");

        let fetched = store.get_proposal(&id).unwrap().expect("proposal exists");
        assert_eq!(fetched.signed_digest, digest);
        assert!(fetched.partials.is_empty());

        let pk = "0x".to_string() + &"11".repeat(96);
        let sig = "0x".to_string() + &"22".repeat(48);
        let partial = PartialDto {
            signer_pk: pk.clone(),
            sig: sig.clone(),
        };
        match store.append_partial(&id, partial).unwrap() {
            AppendOutcome::Appended(dto) => {
                assert_eq!(dto.partials.len(), 1);
                assert_eq!(dto.signed_digest, digest);
            }
            _ => panic!("expected Appended"),
        }

        let dup = PartialDto {
            signer_pk: pk,
            sig,
        };
        matches!(
            store.append_partial(&id, dup).unwrap(),
            AppendOutcome::DuplicatePk
        )
        .then_some(())
        .expect("expected DuplicatePk");

        let summaries = store.list_proposals().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].partials_count, 1);
        assert_eq!(summaries[0].id, id);
    }

    #[test]
    fn append_partial_to_unknown_id_returns_not_found() {
        let store = Store::open_in_memory().expect("open store");
        let partial = PartialDto {
            signer_pk: "0x".to_string() + &"11".repeat(96),
            sig: "0x".to_string() + &"22".repeat(48),
        };
        matches!(
            store.append_partial("deadbeef", partial).unwrap(),
            AppendOutcome::NotFound
        )
        .then_some(())
        .expect("expected NotFound");
    }

    #[test]
    fn create_proposal_is_idempotent_for_identical_body() {
        let store = Store::open_in_memory().expect("open store");
        let digest = "0x".to_string() + &"cd".repeat(32);
        let id = digest.trim_start_matches("0x").to_string();
        let dto = sample_dto(&digest);

        matches!(
            store.create_proposal(&id, &digest, &dto).unwrap(),
            CreateOutcome::Created
        )
        .then_some(())
        .expect("first create should be Created");
        matches!(
            store.create_proposal(&id, &digest, &dto).unwrap(),
            CreateOutcome::AlreadyExists
        )
        .then_some(())
        .expect("second identical create should be AlreadyExists");
    }

    #[test]
    fn create_proposal_conflict_when_same_id_different_body() {
        let store = Store::open_in_memory().expect("open store");
        let digest = "0x".to_string() + &"ef".repeat(32);
        let id = digest.trim_start_matches("0x").to_string();
        let dto = sample_dto(&digest);
        let mut different = dto.clone();
        different.intent.nonce += 1;

        matches!(
            store.create_proposal(&id, &digest, &dto).unwrap(),
            CreateOutcome::Created
        )
        .then_some(())
        .expect("first create should be Created");
        matches!(
            store.create_proposal(&id, &digest, &different).unwrap(),
            CreateOutcome::Conflict
        )
        .then_some(())
        .expect("second create with different body under same id should be Conflict");
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
