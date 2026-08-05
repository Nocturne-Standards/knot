//! HTTP smoke for generic Lab RPC paths after PM peel.
//! Spawns the real `knot-tool serve` binary (mock ledger) and exercises
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
    stderr_buf: std::sync::Arc<std::sync::Mutex<String>>,
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
    let status = Command::new(env!("CARGO_BIN_EXE_knot-tool"))
        .args([
            "--store",
            store.to_str().expect("store path utf8"),
            "identity",
            "new",
            name,
        ])
        .env("KNOT_PWD", pwd)
        .env("KNOT_ALLOW_ENV_PWD", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn identity new");
    assert!(status.success(), "identity new {name} failed");
}

fn spawn_serve(store: &Path, bind: &str, pwd: &str) -> ServeProcess {
    let stderr_buf = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let buf_clone = stderr_buf.clone();
    let mut child = Command::new(env!("CARGO_BIN_EXE_knot-tool"))
        .args([
            "--store",
            store.to_str().expect("store path utf8"),
            "serve",
            "--bind",
            bind,
        ])
        .env("KNOT_PWD", pwd)
        .env("KNOT_ALLOW_ENV_PWD", "1")
        .env("DEMO_MODE", "mock")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    if let Some(mut stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let mut collected = String::new();
            stderr.read_to_string(&mut collected).ok();
            if let Ok(mut guard) = buf_clone.lock() {
                *guard = collected;
            }
        });
    }
    ServeProcess { child, stderr_buf }
}

fn serve_stderr(serve: &ServeProcess) -> String {
    serve.stderr_buf.lock().expect("stderr lock").clone()
}

fn extract_bootstrap_code(stderr: &str) -> String {
    let marker = "code=";
    let start = stderr
        .find(marker)
        .expect("bootstrap code= in serve stderr")
        + marker.len();
    let rest = &stderr[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_hexdigit())
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

async fn wait_for_serve(serve: &mut ServeProcess, client: &reqwest::Client, base: &str) -> String {
    for _ in 0..120 {
        match serve.child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                if let Some(mut stdout) = serve.child.stdout.take() {
                    stdout.read_to_string(&mut out).ok();
                }
                let err = serve_stderr(serve);
                panic!(
                    "serve exited early ({status}): stdout={out} stderr={err}"
                );
            }
            Ok(None) => {}
            Err(e) => panic!("try_wait failed: {e}"),
        }
        if let Ok(resp) = client.get(format!("{base}/")).send().await {
            if resp.status().is_success() {
                for _ in 0..40 {
                    let stderr = serve_stderr(serve);
                    if stderr.contains("code=") {
                        return extract_bootstrap_code(&stderr);
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                panic!(
                    "bootstrap code missing from serve stderr: {}",
                    serve_stderr(serve)
                );
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let err = serve_stderr(serve);
    panic!("serve never answered at {base}/ within 12s; stderr={err}");
}

async fn bootstrap_session(client: &reqwest::Client, base: &str, code: &str) {
    let resp = client
        .get(format!("{base}/?code={code}"))
        .send()
        .await
        .expect("bootstrap");
    assert!(
        resp.status().is_success(),
        "bootstrap final status {}",
        resp.status()
    );
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

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
        .expect("http client");
    let code = wait_for_serve(&mut serve, &client, &base).await;

    let html = client
        .get(format!("{base}/"))
        .send()
        .await
        .expect("index")
        .text()
        .await
        .expect("index html");
    assert!(!html.contains("KNOT_TOOL_TOKEN"));
    assert!(!html.contains("__TOKEN__"));
    assert!(!html.contains(&code));

    let unauthorized = client
        .get(format!("{base}/api/setup/status"))
        .send()
        .await
        .expect("setup without session");
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

    bootstrap_session(&client, &base, &code).await;

    let reuse = client
        .get(format!("{base}/?code={code}"))
        .send()
        .await
        .expect("reuse otp");
    assert_eq!(reuse.status(), reqwest::StatusCode::UNAUTHORIZED);

    let setup = client
        .get(format!("{base}/api/setup/status"))
        .send()
        .await
        .expect("setup status");
    assert!(setup.status().is_success());
    let setup_json: serde_json::Value = setup.json().await.expect("setup json");
    assert_eq!(setup_json["demo_mode"], "mock");
    assert_eq!(setup_json["identities_count"], 3);

    let pm_resolve = client
        .get(format!("{base}/api/pm-resolve/status"))
        .send()
        .await
        .expect("pm-resolve probe");
    assert_eq!(pm_resolve.status(), reqwest::StatusCode::NOT_FOUND);

    let create_account = client
        .post(format!("{base}/api/account/create"))
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
        .send()
        .await
        .expect("next id");
    assert_eq!(next_id.json::<u64>().await.expect("next id json"), 0);

    let target = format!("0x{}", "ab".repeat(32));
    let create_proposal = client
        .post(format!("{base}/api/proposal/create"))
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
        .send()
        .await
        .expect("preview");
    assert!(preview.status().is_success());
    let preview_json: serde_json::Value = preview.json().await.expect("preview json");
    assert_eq!(preview_json["function_name"], "set_value");

    let approve = client
        .post(format!("{base}/api/proposal/0/approve"))
        .json(&serde_json::json!({ "signer": "alice", "confirm": true }))
        .send()
        .await
        .expect("approve");
    assert!(approve.status().is_success());

    let status = client
        .get(format!("{base}/api/proposal/0"))
        .send()
        .await
        .expect("status");
    let status_json: serde_json::Value = status.json().await.expect("status json");
    assert_eq!(status_json["status"], "Open");
    assert_eq!(status_json["approvals_len"], 1);

    let approve_bob = client
        .post(format!("{base}/api/proposal/0/approve"))
        .json(&serde_json::json!({ "signer": "bob", "confirm": true }))
        .send()
        .await
        .expect("approve bob");
    assert!(approve_bob.status().is_success());

    let finalize = client
        .post(format!("{base}/api/proposal/0/finalize"))
        .send()
        .await
        .expect("finalize");
    assert!(finalize.status().is_success());
    let finalize_json: serde_json::Value = finalize.json().await.expect("finalize json");
    assert_eq!(finalize_json["tx_hash"], "mock-finalize-0");

    let final_status = client
        .get(format!("{base}/api/proposal/0"))
        .send()
        .await
        .expect("final status");
    let final_json: serde_json::Value = final_status.json().await.expect("final json");
    assert_eq!(final_json["status"], "Executed");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn approve_rejects_non_member_identity() {
    let pwd = "rpc-generic-smoke-pwd";
    let dir = std::env::temp_dir().join(format!(
        "multisig-rpc-smoke-nonmember-{}-{}",
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

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
        .expect("http client");
    let code = wait_for_serve(&mut serve, &client, &base).await;
    bootstrap_session(&client, &base, &code).await;

    let create_account = client
        .post(format!("{base}/api/account/create"))
        .json(&serde_json::json!({
            "members": ["alice", "bob"],
            "threshold": 2
        }))
        .send()
        .await
        .expect("account create");
    assert!(create_account.status().is_success());

    let target = format!("0x{}", "cd".repeat(32));
    let create_proposal = client
        .post(format!("{base}/api/proposal/create"))
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

    let non_member = client
        .post(format!("{base}/api/proposal/0/approve"))
        .json(&serde_json::json!({ "signer": "carol", "confirm": true }))
        .send()
        .await
        .expect("approve carol");
    assert_eq!(non_member.status(), reqwest::StatusCode::FORBIDDEN);
    let err_body = non_member.text().await.expect("error body");
    assert!(err_body.contains("not a member"));

    let member = client
        .post(format!("{base}/api/proposal/0/approve"))
        .json(&serde_json::json!({ "signer": "alice", "confirm": true }))
        .send()
        .await
        .expect("approve alice");
    assert!(member.status().is_success());

    let _ = std::fs::remove_dir_all(&dir);
}
