//! HTTP smoke for generic Lab RPC paths after PM peel.
//! Spawns the real `multisig-tool serve` binary (mock ledger) and exercises
//! account create → proposal create → preview → approve → finalize.
//!
//! Axum oneshot coverage lives in `rpc::generic_rpc_smoke` (binary unit tests).

use std::io::Read;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct ServeProcess {
    child: Child,
}

impl Drop for ServeProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local addr").port()
}

fn run_identity_new(store: &Path, name: &str, pwd: &str) {
    let status = Command::new(env!("CARGO_BIN_EXE_multisig-tool"))
        .args([
            "--store",
            store.to_str().expect("store path utf8"),
            "identity",
            "new",
            name,
        ])
        .env("MULTISIG_TOOL_PWD", pwd)
        .env("MULTISIG_TOOL_ALLOW_ENV_PWD", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn identity new");
    assert!(status.success(), "identity new {name} failed");
}

fn spawn_serve(store: &Path, bind: &str, pwd: &str) -> ServeProcess {
    let child = Command::new(env!("CARGO_BIN_EXE_multisig-tool"))
        .args([
            "--store",
            store.to_str().expect("store path utf8"),
            "serve",
            "--bind",
            bind,
        ])
        .env("MULTISIG_TOOL_PWD", pwd)
        .env("MULTISIG_TOOL_ALLOW_ENV_PWD", "1")
        .env("DEMO_MODE", "mock")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    ServeProcess { child }
}

async fn wait_for_serve(serve: &mut ServeProcess, client: &reqwest::Client, base: &str) {
    for _ in 0..120 {
        match serve.child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                if let Some(mut stdout) = serve.child.stdout.take() {
                    stdout.read_to_string(&mut out).ok();
                }
                let mut err = String::new();
                if let Some(mut stderr) = serve.child.stderr.take() {
                    stderr.read_to_string(&mut err).ok();
                }
                panic!(
                    "serve exited early ({status}): stdout={out} stderr={err}"
                );
            }
            Ok(None) => {}
            Err(e) => panic!("try_wait failed: {e}"),
        }
        if let Ok(resp) = client.get(format!("{base}/")).send().await {
            if resp.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("serve never answered at {base}/ within 12s");
}

fn extract_token(html: &str) -> String {
    const PREFIX: &str = "window.MULTISIG_TOOL_TOKEN = \"";
    let start = html
        .find(PREFIX)
        .expect("MULTISIG_TOOL_TOKEN in index html")
        + PREFIX.len();
    let end = start + html[start..].find('"').expect("token closing quote");
    html[start..end].to_string()
}

#[tokio::test]
async fn serve_mock_generic_proposal_flow_smoke() {
    let pwd = "rpc-generic-smoke-pwd";
    let dir = std::env::temp_dir().join(format!(
        "multisig-rpc-smoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let store = dir.join("identities.dat");

    for name in ["alice", "bob", "carol"] {
        run_identity_new(&store, name, pwd);
    }

    let port = free_port();
    let bind = format!("127.0.0.1:{port}");
    let base = format!("http://{bind}");
    let mut serve = spawn_serve(&store, &bind, pwd);

    let client = reqwest::Client::new();
    wait_for_serve(&mut serve, &client, &base).await;

    let html = client
        .get(format!("{base}/"))
        .send()
        .await
        .expect("index")
        .text()
        .await
        .expect("index html");
    let token = extract_token(&html);

    let unauthorized = client
        .get(format!("{base}/api/setup/status"))
        .send()
        .await
        .expect("setup without token");
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

    let setup = client
        .get(format!("{base}/api/setup/status"))
        .header("X-Multisig-Tool-Token", &token)
        .send()
        .await
        .expect("setup status");
    assert!(setup.status().is_success());
    let setup_json: serde_json::Value = setup.json().await.expect("setup json");
    assert_eq!(setup_json["demo_mode"], "mock");
    assert_eq!(setup_json["identities_count"], 3);

    let pm_resolve = client
        .get(format!("{base}/api/pm-resolve/status"))
        .header("X-Multisig-Tool-Token", &token)
        .send()
        .await
        .expect("pm-resolve probe");
    assert_eq!(pm_resolve.status(), reqwest::StatusCode::NOT_FOUND);

    let create_account = client
        .post(format!("{base}/api/account/create"))
        .header("X-Multisig-Tool-Token", &token)
        .json(&serde_json::json!({
            "members": ["alice", "bob", "carol"],
            "threshold": 2
        }))
        .send()
        .await
        .expect("account create");
    assert!(create_account.status().is_success());
    let account_out: serde_json::Value = create_account.json().await.expect("account json");
    assert_eq!(account_out["outcome"], "ok");

    let next_id = client
        .get(format!("{base}/api/proposal/next-id"))
        .header("X-Multisig-Tool-Token", &token)
        .send()
        .await
        .expect("next id");
    assert_eq!(next_id.json::<u64>().await.expect("next id json"), 0);

    let target = format!("0x{}", "ab".repeat(32));
    let create_proposal = client
        .post(format!("{base}/api/proposal/create"))
        .header("X-Multisig-Tool-Token", &token)
        .json(&serde_json::json!({
            "account": 0,
            "target": target,
            "function": "set_value",
            "args_hex": "0x0708",
            "deadline": 500
        }))
        .send()
        .await
        .expect("proposal create");
    assert!(create_proposal.status().is_success());

    let preview = client
        .get(format!("{base}/api/proposal/0/preview"))
        .header("X-Multisig-Tool-Token", &token)
        .send()
        .await
        .expect("preview");
    assert!(preview.status().is_success());
    let preview_json: serde_json::Value = preview.json().await.expect("preview json");
    assert_eq!(preview_json["function_name"], "set_value");

    let approve = client
        .post(format!("{base}/api/proposal/0/approve"))
        .header("X-Multisig-Tool-Token", &token)
        .json(&serde_json::json!({ "signer": "alice", "confirm": true }))
        .send()
        .await
        .expect("approve");
    assert!(approve.status().is_success());

    let status = client
        .get(format!("{base}/api/proposal/0"))
        .header("X-Multisig-Tool-Token", &token)
        .send()
        .await
        .expect("status");
    let status_json: serde_json::Value = status.json().await.expect("status json");
    assert_eq!(status_json["status"], "Open");
    assert_eq!(status_json["approvals_len"], 1);

    let approve_bob = client
        .post(format!("{base}/api/proposal/0/approve"))
        .header("X-Multisig-Tool-Token", &token)
        .json(&serde_json::json!({ "signer": "bob", "confirm": true }))
        .send()
        .await
        .expect("approve bob");
    assert!(approve_bob.status().is_success());

    let finalize = client
        .post(format!("{base}/api/proposal/0/finalize"))
        .header("X-Multisig-Tool-Token", &token)
        .send()
        .await
        .expect("finalize");
    assert!(finalize.status().is_success());
    let finalize_json: serde_json::Value = finalize.json().await.expect("finalize json");
    assert_eq!(finalize_json["tx_hash"], "mock-finalize-0");

    let final_status = client
        .get(format!("{base}/api/proposal/0"))
        .header("X-Multisig-Tool-Token", &token)
        .send()
        .await
        .expect("final status");
    let final_json: serde_json::Value = final_status.json().await.expect("final json");
    assert_eq!(final_json["status"], "Executed");

    let _ = std::fs::remove_dir_all(&dir);
}
