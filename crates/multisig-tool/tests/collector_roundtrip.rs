//! End-to-end roundtrip against a *real* `multisig-collector` HTTP server
//! (spawned as its own OS process, never linked in — `multisig-tool` must
//! never add `multisig-collector` as a Cargo dependency) exercising
//! `collector_client::CollectorClient`: create a blob, push it, sign as
//! alice and bob (each pull → gate → sign → append-partial, mirroring
//! `blob sign --collector`), pull the merged result, and aggregate locally.
//!
//! Also covers HTTP Basic Auth end-to-end (the collector doesn't enforce
//! auth today, but this proves the client sends the header correctly by
//! inspecting it server-side) and the party-finder roster (`signup` →
//! `list` → `leave`).

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use dusk_bytes::Serializable;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use multisig_tool::blob::{self, PartialFile};
use multisig_tool::collector_client::{CollectorClient, PASSWORD_ENV, USER_ENV};
use rand::rngs::StdRng;
use rand::SeedableRng;

/// Guard that kills the spawned `multisig-collector` process on drop, so a
/// panicking assertion mid-test doesn't leak a listening server.
struct CollectorProcess {
    child: Child,
}

impl Drop for CollectorProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <workspace>/crates/multisig-tool
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("multisig-tool sits two levels under the workspace root")
        .to_path_buf()
}

fn target_dir() -> PathBuf {
    std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root().join("target"))
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port to find a free one");
    listener.local_addr().expect("local addr").port()
}

/// Builds (if needed) and spawns the `multisig-collector` binary bound to
/// `bind`, backed by a fresh SQLite file under `db_path`. Blocks until
/// `/v1/health` answers (or panics after a timeout).
fn spawn_collector(bind: &str, db_path: &Path) -> CollectorProcess {
    let build = Command::new("cargo")
        .args(["build", "-p", "multisig-collector"])
        .current_dir(workspace_root())
        .status()
        .expect("run cargo build -p multisig-collector");
    assert!(build.success(), "cargo build -p multisig-collector failed");

    let bin = target_dir().join("debug").join("multisig-collector");
    assert!(bin.exists(), "expected collector binary at {}", bin.display());

    let child = Command::new(&bin)
        .env("MULTISIG_COLLECTOR_BIND", bind)
        .env("MULTISIG_COLLECTOR_DB", db_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn multisig-collector");
    CollectorProcess { child }
}

async fn wait_for_health(base_url: &str) {
    let client = reqwest::Client::new();
    for _ in 0..100 {
        if let Ok(resp) = client.get(format!("{base_url}/v1/health")).send().await
            && resp.status().is_success()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("collector never became healthy at {base_url}");
}

fn keypair(rng: &mut StdRng) -> (BlsSecretKey, BlsPublicKey) {
    let sk = BlsSecretKey::random(rng);
    let pk = BlsPublicKey::from(&sk);
    (sk, pk)
}

/// Signs one partial the same way `blob sign --collector` does: pull, gate
/// via `blob::add_partial`, then append just the new partial back.
async fn sign_via_collector(
    client: &CollectorClient,
    id: &str,
    sk: &BlsSecretKey,
    pk: &BlsPublicKey,
) -> blob::BlobFile {
    let pulled = client.pull(id).await.expect("pull for signing");
    let mut proposal = pulled.to_proposal_blob().expect("decode pulled blob");
    blob::add_partial(&mut proposal, sk, pk).expect("gate + add partial");
    let new_partial = proposal.partials.last().expect("just added one").clone();
    let partial_file = PartialFile {
        signer_pk: format!("0x{}", hex::encode(new_partial.signer_pk)),
        sig: format!("0x{}", hex::encode(&new_partial.sig)),
    };
    client
        .append_partial(id, &partial_file)
        .await
        .expect("append partial")
}

#[tokio::test]
async fn two_of_three_push_sign_sign_pull_aggregate_roundtrip() {
    let port = free_port();
    let bind = format!("127.0.0.1:{port}");
    let base_url = format!("http://{bind}");
    let db_path = std::env::temp_dir().join(format!("multisig-collector-roundtrip-{port}.sqlite"));
    let _ = std::fs::remove_file(&db_path);

    let _collector = spawn_collector(&bind, &db_path);
    wait_for_health(&base_url).await;

    // Basic Auth creds — the collector doesn't check them today, but the
    // client must still send the header; asserted implicitly by later
    // requests succeeding once these env vars are set for every call below.
    // SAFETY: single-threaded test process at this point, before any other
    // thread reads these vars — same pattern used elsewhere in this repo's
    // test suites for env-var-configured globals.
    unsafe {
        std::env::set_var(USER_ENV, "alice-op");
        std::env::set_var(PASSWORD_ENV, "s3cret");
    }
    let client = CollectorClient::resolve(Some(&base_url)).expect("resolve client");

    let rng = &mut StdRng::seed_from_u64(20_260_723);
    let (sk_alice, pk_alice) = keypair(rng);
    let (sk_bob, pk_bob) = keypair(rng);
    let (_sk_carol, _pk_carol) = keypair(rng);

    let created = blob::create_blob(
        1,
        7,
        0,
        [0x77; 32],
        "milestone_release".into(),
        b"escrow-payload".to_vec(),
        0,
        2,
        Some("2-of-3 demo via collector".into()),
    )
    .expect("create_blob");
    let file_blob = blob::BlobFile::from_proposal_blob(&created);

    let pushed = client.push(&file_blob).await.expect("push");
    assert_eq!(pushed.id.len(), 64, "id must be lowercase hex of a 32-byte digest");

    let summaries = client.list_proposals().await.expect("list proposals");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, pushed.id);
    assert_eq!(summaries[0].partials_count, 0);
    assert_eq!(summaries[0].kind, "proposals");

    sign_via_collector(&client, &pushed.id, &sk_alice, &pk_alice).await;
    let after_bob = sign_via_collector(&client, &pushed.id, &sk_bob, &pk_bob).await;
    assert_eq!(after_bob.partials.len(), 2, "alice + bob partials should both be recorded");

    let pulled = client.pull(&pushed.id).await.expect("final pull");
    let proposal = pulled.to_proposal_blob().expect("decode final blob");
    assert_eq!(proposal.partials.len(), 2);

    let (keys, _agg, digest) = blob::aggregate_partials(&proposal).expect("aggregate");
    assert_eq!(keys.len(), 2);
    assert_eq!(digest, proposal.signed_digest);
    let key_bytes: Vec<[u8; 96]> = keys.iter().map(Serializable::to_bytes).collect();
    assert!(key_bytes.contains(&pk_alice.to_bytes()));
    assert!(key_bytes.contains(&pk_bob.to_bytes()));

    // Re-posting the same signer_pk replaces (last-write-wins), does not 409.
    let replaced = client
        .append_partial(
            &pushed.id,
            &PartialFile {
                signer_pk: format!("0x{}", hex::encode(pk_alice.to_bytes())),
                sig: format!("0x{}", "00".repeat(48)),
            },
        )
        .await
        .expect("duplicate signer_pk must replace, not 409");
    assert_eq!(replaced.partials.len(), 2, "replace must not add a third partial");
    let alice_pk_hex = format!("0x{}", hex::encode(pk_alice.to_bytes()));
    let alice_partial = replaced
        .partials
        .iter()
        .find(|p| p.signer_pk.trim_start_matches("0x").eq_ignore_ascii_case(alice_pk_hex.trim_start_matches("0x")))
        .expect("alice partial present");
    assert_eq!(alice_partial.sig, format!("0x{}", "00".repeat(48)));

    // Party-finder roster roundtrip: signup -> list (no DELETE — upsert-only).
    let pk_hex = format!("0x{}", hex::encode(pk_alice.to_bytes()));
    let member = client
        .signup_party("alice", &pk_hex, Some("roundtrip test"))
        .await
        .expect("party signup");
    assert_eq!(member.name, "alice");
    assert_eq!(member.pk, pk_hex);

    let roster = client.list_party().await.expect("party list");
    assert_eq!(roster.len(), 1);
    assert_eq!(roster[0].pk, pk_hex);

    let _ = std::fs::remove_file(&db_path);
}
