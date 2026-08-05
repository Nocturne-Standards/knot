// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 Nocturne Standards

//! SQLite-backed store: proposals + party roster.
//!
//! `proposals` holds one row per content-addressed proposal id. Partials
//! are **not** a separate table — the whole `ProposalDto` (intent +
//! signed_digest + threshold + partials) lives in a single `body_json`
//! column, rewritten atomically (single `UPDATE`, under the same connection
//! mutex) on every partial append/replace. This makes "never mutate
//! `signed_digest`" trivial: the append path deserializes the stored blob,
//! updates `partials` only, and re-serializes — there is no code path that
//! touches `signed_digest` or `intent`.

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use crate::dto::{PartialDto, PartyMemberDto, ProposalDto, ProposalSummary};
use crate::{MAX_PARTY_ROWS, MAX_PARTIALS, MAX_PROPOSAL_ROWS, PROPOSAL_RETENTION_SECS};

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
    /// A proposal with the same identity (version/intent/digest/threshold)
    /// already existed — create is idempotent even if partials have since
    /// been appended (create payloads always arrive with `partials: []`).
    AlreadyExists,
    /// A row already exists under this id with a **different** identity.
    /// The id is the lowercase hex of `signed_digest` (content-addressed, not
    /// a hash collision) — a different intent under the same digest is an
    /// attack or client bug, never a normal path.
    Conflict,
    /// Table row cap reached (M11).
    RowCapReached,
}

/// Result of `Store::append_partial`.
pub enum AppendOutcome {
    /// Partial appended or replaced; carries the full updated blob.
    Appended(ProposalDto),
    /// No proposal exists under this id.
    NotFound,
    /// A *new* `signer_pk` would push past [`crate::MAX_PARTIALS`].
    /// Replacing an existing pk never hits this.
    TooManyPartials,
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
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA fullfsync = ON;",
        )
        .context("set sqlite durability pragmas")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// In-memory store for tests — no file on disk.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory sqlite database")?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA fullfsync = ON;",
        )
        .context("set sqlite durability pragmas")?;
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
            );
            CREATE TABLE IF NOT EXISTS party (
                pk TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                note TEXT,
                joined_at INTEGER NOT NULL
            );",
        )
        .context("create proposals and party tables")?;
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
        self.sweep_expired()?;
        let conn = self.conn.lock().expect("db mutex poisoned");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM proposals", [], |row| row.get(0))
            .context("count proposals")?;
        if count as usize >= MAX_PROPOSAL_ROWS {
            return Ok(CreateOutcome::RowCapReached);
        }
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
            // Idempotency ignores `partials`: create always arrives with an
            // empty list, while the store may already hold co-signer partials.
            // Conflict only when intent/digest/threshold/version disagree.
            return Ok(if proposal_identity_eq(&existing_dto, dto) {
                CreateOutcome::AlreadyExists
            } else {
                tracing::warn!(
                    id = id,
                    existing_version = existing_dto.version,
                    new_version = dto.version,
                    existing_kind = ?existing_dto.kind,
                    new_kind = ?dto.kind,
                    "content-addressed id conflict: different intent under same digest"
                );
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

    /// Summaries for `GET /v1/proposals`, most recently created first (M11 pagination).
    pub fn list_proposals(&self, limit: u32, offset: u32) -> Result<Vec<ProposalSummary>> {
        self.sweep_expired()?;
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, digest, body_json, created_at FROM proposals \
                 ORDER BY created_at DESC, id ASC LIMIT ?1 OFFSET ?2",
            )
            .context("prepare list query")?;
        let mut rows = stmt
            .query(params![limit, offset])
            .context("run list query")?;
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
                kind: dto.kind,
                threshold: dto.threshold,
                partials_count: dto.partials.len(),
                created_at,
            });
        }
        Ok(out)
    }

    /// Appends one partial, or **replaces** the existing entry for the same
    /// `signer_pk` (last-write-wins). Never mutates `signed_digest` /
    /// `intent`. Cap: a *new* pk is rejected once [`MAX_PARTIALS`] are
    /// already stored; replacing an existing pk is always allowed.
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
        let digest_before = dto.signed_digest.clone();

        if let Some(existing) = dto.partials.iter_mut().find(|p| {
            p.signer_pk.trim_start_matches("0x").to_ascii_lowercase() == new_pk_norm
        }) {
            existing.sig = partial.sig;
            // Keep stored signer_pk as already-normalized from the first insert.
        } else {
            if dto.partials.len() >= MAX_PARTIALS {
                return Ok(AppendOutcome::TooManyPartials);
            }
            dto.partials.push(partial);
        }

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

    /// Inserts a new roster row under `pk`, or — if `pk` already has a row —
    /// overwrites its `name`/`note` while leaving `joined_at` at the
    /// original signup time (a re-signup refreshes displayed info, it
    /// doesn't reset "when this person joined").
    pub fn upsert_party_member(
        &self,
        pk: &str,
        name: &str,
        note: Option<&str>,
    ) -> Result<PartyMemberDto> {
        self.sweep_expired()?;
        let conn = self.conn.lock().expect("db mutex poisoned");
        let existing_joined_at: Option<i64> = conn
            .query_row(
                "SELECT joined_at FROM party WHERE pk = ?1",
                params![pk],
                |row| row.get(0),
            )
            .optional()
            .context("query existing party member")?;
        if existing_joined_at.is_none() {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM party", [], |row| row.get(0))
                .context("count party rows")?;
            if count as usize >= MAX_PARTY_ROWS {
                anyhow::bail!("party roster row cap reached");
            }
        }
        let joined_at = existing_joined_at.unwrap_or_else(now_unix);
        conn.execute(
            "INSERT INTO party (pk, name, note, joined_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(pk) DO UPDATE SET name = excluded.name, note = excluded.note",
            params![pk, name, note, joined_at],
        )
        .context("upsert party member")?;
        Ok(PartyMemberDto {
            name: name.to_string(),
            pk: pk.to_string(),
            note: note.map(str::to_string),
            joined_at,
        })
    }

    /// Roster for `GET /v1/party`, earliest signup first (M11 pagination).
    pub fn list_party(&self, limit: u32, offset: u32) -> Result<Vec<PartyMemberDto>> {
        self.sweep_expired()?;
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT pk, name, note, joined_at FROM party \
                 ORDER BY joined_at ASC, pk ASC LIMIT ?1 OFFSET ?2",
            )
            .context("prepare party list query")?;
        let mut rows = stmt
            .query(params![limit, offset])
            .context("run party list query")?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().context("step party list query")? {
            out.push(PartyMemberDto {
                pk: row.get(0)?,
                name: row.get(1)?,
                note: row.get(2)?,
                joined_at: row.get(3)?,
            });
        }
        Ok(out)
    }

    /// Deletes proposal rows older than [`PROPOSAL_RETENTION_SECS`] (M11 TTL).
    fn sweep_expired(&self) -> Result<()> {
        let cutoff = now_unix() - PROPOSAL_RETENTION_SECS;
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.execute("DELETE FROM proposals WHERE created_at < ?1", params![cutoff])
            .context("sweep expired proposals")?;
        Ok(())
    }

}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Proposal identity for create idempotency — ignores `partials`.
fn proposal_identity_eq(a: &ProposalDto, b: &ProposalDto) -> bool {
    a.version == b.version
        && a.kind == b.kind
        && a.intent == b.intent
        && a.signed_digest == b.signed_digest
        && a.threshold == b.threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{BlobKind, IntentDto, ProposalsIntentDto};

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
            kind: BlobKind::Proposals,
            intent: IntentDto::Proposals(ProposalsIntentDto {
                chain_id: 1,
                committee_id: 7,
                nonce: 3,
                target_contract_id: "0x11".to_string(),
                function_name: "set_service".to_string(),
                call_args: "0x0001".to_string(),
                deadline: 1000,
                human_summary: Some("hint".to_string()),
            }),
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
            sig: "0x".to_string() + &"33".repeat(48),
        };
        match store.append_partial(&id, dup).unwrap() {
            AppendOutcome::Appended(dto) => {
                assert_eq!(dto.partials.len(), 1, "replace must not add a second row");
                assert_eq!(dto.partials[0].sig, "0x".to_string() + &"33".repeat(48));
                assert_eq!(dto.signed_digest, digest);
            }
            _ => panic!("expected Appended (last-write-wins replace)"),
        }

        let summaries = store.list_proposals(50, 0).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].partials_count, 1);
        assert_eq!(summaries[0].id, id);
    }

    #[test]
    fn append_rejects_33rd_distinct_pk() {
        let store = Store::open_in_memory().expect("open store");
        let digest = "0x".to_string() + &"aa".repeat(32);
        let id = digest.trim_start_matches("0x").to_string();
        let dto = sample_dto(&digest);
        store.create_proposal(&id, &digest, &dto).unwrap();

        for i in 0..crate::MAX_PARTIALS {
            let pk = format!("0x{}", hex::encode(vec![i as u8; 96]));
            let sig = format!("0x{}", hex::encode(vec![0x22u8; 48]));
            match store
                .append_partial(
                    &id,
                    PartialDto {
                        signer_pk: pk,
                        sig,
                    },
                )
                .unwrap()
            {
                AppendOutcome::Appended(_) => {}
                _ => panic!("expected Appended for pk #{i}"),
            }
        }

        let overflow_pk = format!("0x{}", hex::encode(vec![0xffu8; 96]));
        matches!(
            store
                .append_partial(
                    &id,
                    PartialDto {
                        signer_pk: overflow_pk,
                        sig: format!("0x{}", hex::encode(vec![0x22u8; 48])),
                    },
                )
                .unwrap(),
            AppendOutcome::TooManyPartials
        )
        .then_some(())
        .expect("33rd distinct pk must be TooManyPartials");

        // Replacing an existing pk still works at the cap.
        let first_pk = format!("0x{}", hex::encode(vec![0u8; 96]));
        match store
            .append_partial(
                &id,
                PartialDto {
                    signer_pk: first_pk,
                    sig: format!("0x{}", hex::encode(vec![0x44u8; 48])),
                },
            )
            .unwrap()
        {
            AppendOutcome::Appended(dto) => {
                assert_eq!(dto.partials.len(), crate::MAX_PARTIALS);
                assert_eq!(
                    dto.partials[0].sig,
                    format!("0x{}", hex::encode(vec![0x44u8; 48]))
                );
            }
            _ => panic!("replace at cap must succeed"),
        }
    }

    fn sample_pm_dto(digest: &str) -> ProposalDto {
        ProposalDto {
            version: 2,
            kind: BlobKind::PmCouncilResolve,
            intent: IntentDto::PmCouncilResolve(crate::dto::PmCouncilResolveIntentDto {
                market_id: 0,
                winning_outcome: 1,
                pm_contract_id: format!("0x{}", "ab".repeat(32)),
                registry_account_id: 0,
                human_summary: Some("pm smoke".into()),
            }),
            signed_digest: digest.to_string(),
            threshold: 2,
            partials: Vec::new(),
        }
    }

    #[test]
    fn pm_create_get_append_round_trip() {
        let store = Store::open_in_memory().expect("open store");
        let digest = "0x".to_string() + &"99".repeat(32);
        let id = digest.trim_start_matches("0x").to_string();
        let dto = sample_pm_dto(&digest);

        matches!(
            store.create_proposal(&id, &digest, &dto).unwrap(),
            CreateOutcome::Created
        )
        .then_some(())
        .expect("expected Created");

        let fetched = store.get_proposal(&id).unwrap().expect("proposal exists");
        assert_eq!(fetched.kind, BlobKind::PmCouncilResolve);
        assert_eq!(fetched.signed_digest, digest);
        match &fetched.intent {
            IntentDto::PmCouncilResolve(i) => {
                assert_eq!(i.market_id, 0);
                assert_eq!(i.winning_outcome, 1);
            }
            IntentDto::Proposals(_) => panic!("expected pm intent"),
        }

        let pk = "0x".to_string() + &"11".repeat(96);
        let sig = "0x".to_string() + &"22".repeat(48);
        match store
            .append_partial(
                &id,
                PartialDto {
                    signer_pk: pk,
                    sig,
                },
            )
            .unwrap()
        {
            AppendOutcome::Appended(dto) => {
                assert_eq!(dto.partials.len(), 1);
                assert_eq!(dto.kind, BlobKind::PmCouncilResolve);
            }
            _ => panic!("expected Appended"),
        }
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
    fn create_after_partial_append_is_still_idempotent() {
        let store = Store::open_in_memory().expect("open store");
        let digest = "0x".to_string() + &"ab".repeat(32);
        let id = digest.trim_start_matches("0x").to_string();
        let dto = sample_dto(&digest);

        matches!(
            store.create_proposal(&id, &digest, &dto).unwrap(),
            CreateOutcome::Created
        )
        .then_some(())
        .expect("first create");

        let partial = PartialDto {
            signer_pk: "0x".to_string() + &"11".repeat(96),
            sig: "0x".to_string() + &"22".repeat(48),
        };
        matches!(
            store.append_partial(&id, partial).unwrap(),
            AppendOutcome::Appended(_)
        )
        .then_some(())
        .expect("append");

        // Re-create with empty partials (normal push payload) must not 409.
        matches!(
            store.create_proposal(&id, &digest, &dto).unwrap(),
            CreateOutcome::AlreadyExists
        )
        .then_some(())
        .expect("re-create after partials should be AlreadyExists");
        let fetched = store.get_proposal(&id).unwrap().expect("still there");
        assert_eq!(fetched.partials.len(), 1, "partials must be preserved");
    }

    #[test]
    fn create_proposal_conflict_when_same_id_different_body() {
        let store = Store::open_in_memory().expect("open store");
        let digest = "0x".to_string() + &"ef".repeat(32);
        let id = digest.trim_start_matches("0x").to_string();
        let dto = sample_dto(&digest);
        let mut different = dto.clone();
        match &mut different.intent {
            IntentDto::Proposals(i) => i.nonce += 1,
            IntentDto::PmCouncilResolve(_) => panic!("sample_dto is proposals"),
        }

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

    #[test]
    fn party_signup_visible_on_second_list() {
        let store = Store::open_in_memory().expect("open store");
        let pk = "0x".to_string() + &"11".repeat(96);

        store
            .upsert_party_member(&pk, "Alice", Some("first signup"))
            .expect("upsert");
        let first_list = store.list_party(50, 0).expect("list");
        assert_eq!(first_list.len(), 1);
        assert_eq!(first_list[0].name, "Alice");

        let second_list = store.list_party(50, 0).expect("list again");
        assert_eq!(second_list.len(), 1);
        assert_eq!(second_list[0].pk, pk);
    }

    #[test]
    fn party_upsert_updates_name_and_keeps_joined_at() {
        let store = Store::open_in_memory().expect("open store");
        let pk = "0x".to_string() + &"22".repeat(96);

        let first = store
            .upsert_party_member(&pk, "Alice", None)
            .expect("first upsert");
        let second = store
            .upsert_party_member(&pk, "Alice Renamed", Some("now with a note"))
            .expect("second upsert");

        assert_eq!(second.name, "Alice Renamed");
        assert_eq!(second.note.as_deref(), Some("now with a note"));
        assert_eq!(second.joined_at, first.joined_at);

        let list = store.list_party(50, 0).expect("list");
        assert_eq!(list.len(), 1, "upsert must not create a duplicate row");
        assert_eq!(list[0].name, "Alice Renamed");
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "knot-collector-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        dir
    }
}
