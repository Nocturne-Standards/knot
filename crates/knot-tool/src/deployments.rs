//! Local reader for `deployments/testnet.json` contract pin files.
//!
//! Two shapes load: the flat root `testnet.json` (contract name → record, plus
//! non-contract keys such as `wiring`) and the enveloped `duskds/testnet.json`
//! (`schema: "nocturne.pins.v1"`, records under `contracts`, names under
//! `aliases`). Any top-level object with a `current` field counts as a record
//! in either shape; `contracts` wins on a name clash.
//!
//! Default build uses this module. Enable `deployments-crate` to delegate to
//! the optional `nocturne-deployments` git dependency instead.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

/// One recorded deployment (current or history row).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DeploymentEntry {
    pub contract_id: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub tx_id: Option<String>,
    #[serde(default)]
    pub wasm_path: Option<String>,
    #[serde(default)]
    pub dd_wasm_path: Option<String>,
    #[serde(default)]
    pub wasm_sha256: Option<String>,
    #[serde(default)]
    pub deploy_nonce: Option<u64>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub deployed_at: Option<String>,
}

/// Per-contract pin: live `current` plus optional history.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ContractRecord {
    pub current: DeploymentEntry,
    #[serde(default)]
    pub history: Vec<DeploymentEntry>,
}

/// Schema tag of the enveloped pin file.
pub const PINS_SCHEMA_V1: &str = "nocturne.pins.v1";

/// Full pin file, normalized to contract name → record.
#[derive(Debug, Clone)]
pub struct DeploymentsFile {
    pub contracts: HashMap<String, ContractRecord>,
    /// `aliases` of the enveloped shape: alias → contract name.
    pub aliases: HashMap<String, String>,
    /// Records that have `current` but did not parse, with the reason; reported on lookup.
    invalid: HashMap<String, String>,
    path: PathBuf,
}

impl DeploymentsFile {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn current(&self, key: &str) -> Result<&DeploymentEntry> {
        let name = self.aliases.get(key).map(String::as_str).unwrap_or(key);
        if let Some(r) = self.contracts.get(name) {
            return Ok(&r.current);
        }
        if let Some(why) = self.invalid.get(name) {
            return Err(anyhow!(
                "`{name}` in {} does not parse: {why}",
                self.path.display()
            ));
        }
        Err(anyhow!("no `{name}.current` in {}", self.path.display()))
    }

    /// Normalize either pin shape (see module docs).
    pub fn from_json(raw: &str, path: PathBuf) -> Result<Self> {
        let root: serde_json::Value = serde_json::from_str(raw)?;
        let obj = root
            .as_object()
            .ok_or_else(|| anyhow!("pin file is not a JSON object"))?;
        if let Some(schema) = obj.get("schema").and_then(|v| v.as_str())
            && schema != PINS_SCHEMA_V1
        {
            return Err(anyhow!(
                "unsupported pin schema `{schema}` (expected {PINS_SCHEMA_V1})"
            ));
        }
        let mut file = DeploymentsFile {
            contracts: HashMap::new(),
            aliases: HashMap::new(),
            invalid: HashMap::new(),
            path,
        };
        // Top level first, so the `contracts` envelope overrides on a clash.
        file.add_records(obj);
        if let Some(contracts) = obj.get("contracts") {
            let contracts = contracts
                .as_object()
                .ok_or_else(|| anyhow!("`contracts` is not an object"))?;
            file.add_records(contracts);
        }
        if let Some(aliases) = obj.get("aliases") {
            file.aliases = serde_json::from_value(aliases.clone())
                .map_err(|e| anyhow!("`aliases` is not a name map: {e}"))?;
        }
        Ok(file)
    }

    fn add_records(&mut self, obj: &serde_json::Map<String, serde_json::Value>) {
        for (name, v) in obj {
            if v.get("current").is_none() {
                continue;
            }
            match serde_json::from_value::<ContractRecord>(v.clone()) {
                Ok(r) => {
                    self.invalid.remove(name);
                    self.contracts.insert(name.clone(), r);
                }
                Err(e) => {
                    self.contracts.remove(name);
                    self.invalid.insert(name.clone(), e.to_string());
                }
            }
        }
    }

    pub fn contract_id(&self, key: &str) -> Result<&str> {
        Ok(self.current(key)?.contract_id.as_str())
    }
}

/// Resolve pin file path (does not read).
///
/// Order: `NOCTURNE_DEPLOYMENTS` (a file, or a dir: `duskds/testnet.json`
/// before `testnet.json`) → walk-up pin dirs → sibling pin dirs.
pub fn resolve_path(start: &Path) -> Result<PathBuf> {
    if let Ok(raw) = env::var("NOCTURNE_DEPLOYMENTS") {
        return resolve_env_path(Path::new(&raw));
    }

    const PIN_RELS: &[&str] = &[
        "deployments/testnet.json",
        "nocturne-deployments/duskds/testnet.json",
        "nocturne-deployments/testnet.json",
    ];
    for dir in start.ancestors() {
        for rel in PIN_RELS {
            let nested = dir.join(rel);
            if nested.is_file() {
                return Ok(nested);
            }
        }
        if let Some(parent) = dir.parent() {
            for rel in PIN_RELS {
                let sibling = parent.join(rel);
                if sibling.is_file() {
                    return Ok(sibling);
                }
            }
        }
    }
    bail_not_found()
}

/// `NOCTURNE_DEPLOYMENTS` target: a pin file, or a pins repo dir.
pub fn resolve_env_path(p: &Path) -> Result<PathBuf> {
    if p.is_file() {
        return Ok(p.to_path_buf());
    }
    if p.is_dir() {
        for rel in ["duskds/testnet.json", "testnet.json"] {
            let file = p.join(rel);
            if file.is_file() {
                return Ok(file);
            }
        }
    }
    bail_not_found()
}

fn bail_not_found() -> Result<PathBuf> {
    Err(anyhow!(
        "deployments file not found (set NOCTURNE_DEPLOYMENTS or place deployments/testnet.json)"
    ))
}

pub fn load(path: impl AsRef<Path>) -> Result<DeploymentsFile> {
    let path = path.as_ref().to_path_buf();
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    DeploymentsFile::from_json(&raw, path.clone())
        .with_context(|| format!("parsing {}", path.display()))
}

pub fn load_from(start: &Path) -> Result<DeploymentsFile> {
    load(resolve_path(start)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn loads_contract_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("testnet.json");
        fs::write(
            &path,
            r#"{
              "knot-registry": {
                "current": { "contract_id": "abc123" },
                "history": []
              }
            }"#,
        )
        .unwrap();
        let file = load(&path).unwrap();
        assert_eq!(file.contract_id("knot-registry").unwrap(), "abc123");
        assert!(file.contract_id("missing").is_err());
    }

    const FLAT: &str = r#"{
      "knot-registry": { "current": { "contract_id": "flatreg" }, "history": [] },
      "knot-proposals": { "current": { "contract_id": "flatprop" } },
      "wiring": { "knot-proposals.init_registry": { "tx_id": "t1" } },
      "broken": { "current": { "version": "no id" } }
    }"#;

    const ENVELOPED: &str = r#"{
      "schema": "nocturne.pins.v1",
      "layer": "duskds",
      "network": "testnet",
      "gas_policy": { "pay_gas_limit": 1 },
      "rpc": "https://testnet.nodes.dusk.network",
      "explorer": null,
      "aliases": { "multisig-registry": "knot-registry" },
      "contracts": {
        "knot-registry": { "current": { "contract_id": "envreg", "version": "0.2.1" }, "history": [] }
      },
      "wiring": { "knot-proposals.init_registry": { "tx_id": "t1" } },
      "vigil-vault": { "current": { "contract_id": "vigil" }, "history": [] }
    }"#;

    #[test]
    fn loads_flat_shape_with_extra_keys() {
        let f = DeploymentsFile::from_json(FLAT, PathBuf::from("flat.json")).unwrap();
        assert_eq!(f.contract_id("knot-registry").unwrap(), "flatreg");
        assert_eq!(f.contract_id("knot-proposals").unwrap(), "flatprop");
        assert!(f.contract_id("wiring").is_err());
        let err = f.contract_id("broken").unwrap_err().to_string();
        assert!(err.contains("does not parse"), "{err}");
    }

    #[test]
    fn loads_enveloped_shape() {
        let f = DeploymentsFile::from_json(ENVELOPED, PathBuf::from("env.json")).unwrap();
        assert_eq!(f.contract_id("knot-registry").unwrap(), "envreg");
        assert_eq!(f.contract_id("multisig-registry").unwrap(), "envreg");
        assert_eq!(f.contract_id("vigil-vault").unwrap(), "vigil");
        assert!(f.contract_id("knot-proposals").is_err());
        assert!(f.contract_id("schema").is_err());
    }

    #[test]
    fn envelope_wins_over_top_level() {
        let raw = r#"{
          "schema": "nocturne.pins.v1",
          "knot-registry": { "current": { "contract_id": "top" } },
          "contracts": { "knot-registry": { "current": { "contract_id": "inner" } } }
        }"#;
        let f = DeploymentsFile::from_json(raw, PathBuf::new()).unwrap();
        assert_eq!(f.contract_id("knot-registry").unwrap(), "inner");
    }

    #[test]
    fn refuses_unknown_schema() {
        let raw = r#"{ "schema": "nocturne.pins.v9", "contracts": {} }"#;
        assert!(DeploymentsFile::from_json(raw, PathBuf::new()).is_err());
    }

    #[test]
    fn env_dir_prefers_duskds_then_root() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("testnet.json"), FLAT).unwrap();
        assert_eq!(
            resolve_env_path(root.path()).unwrap(),
            root.path().join("testnet.json")
        );
        fs::create_dir_all(root.path().join("duskds")).unwrap();
        let env_file = root.path().join("duskds").join("testnet.json");
        fs::write(&env_file, ENVELOPED).unwrap();
        assert_eq!(resolve_env_path(root.path()).unwrap(), env_file);
        // a file path is taken as is
        let flat = root.path().join("testnet.json");
        assert_eq!(resolve_env_path(&flat).unwrap(), flat);
        assert_eq!(
            load(resolve_env_path(root.path()).unwrap())
                .unwrap()
                .contract_id("knot-registry")
                .unwrap(),
            "envreg"
        );
        let empty = tempfile::tempdir().unwrap();
        assert!(resolve_env_path(empty.path()).is_err());
        assert!(resolve_env_path(&empty.path().join("missing.json")).is_err());
    }

    #[test]
    fn resolve_sibling_deployments() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: serialized by ENV_LOCK; test-only
        unsafe { env::remove_var("NOCTURNE_DEPLOYMENTS") };

        let root = tempfile::tempdir().unwrap();
        let pins = root.path().join("deployments");
        fs::create_dir_all(&pins).unwrap();
        fs::write(
            pins.join("testnet.json"),
            r#"{"k":{"current":{"contract_id":"id1"}}}"#,
        )
        .unwrap();
        let product = root.path().join("knot").join("crates").join("tool");
        fs::create_dir_all(&product).unwrap();

        let resolved = resolve_path(&product).unwrap();
        assert_eq!(resolved, pins.join("testnet.json"));
        assert_eq!(
            load_from(&product).unwrap().contract_id("k").unwrap(),
            "id1"
        );
    }
}
