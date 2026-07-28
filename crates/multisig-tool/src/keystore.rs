//! Encrypted local store of named BLS "member" identities — the keys a demo
//! user signs multisig-registry quorum messages with. Deliberately NOT
//! `rusk-wallet`'s `.dat` wallet format (that's a single BIP39-seed wallet,
//! wrong shape for "N independently-named keypairs") — this reuses the same
//! class of vetted crates (`aes-gcm`, `pbkdf2`) rather than inventing crypto,
//! scoped to our actual data shape.
//!
//! File layout: `[salt: 16 bytes][nonce: 12 bytes][AES-256-GCM ciphertext]`.
//! Ciphertext plaintext is JSON:
//! `[{"name": ..., "sk_hex": ...?, "pk_hex": ...?}, ...]`.
//! - Full identity: `sk_hex` set (pk derived); `pk_hex` optional.
//! - PK-only: `sk_hex` absent/null, `pk_hex` required — usable as account
//!   members but not as `--signer`.
//! Password never touches disk; key = PBKDF2-HMAC-SHA256(password, salt,
//! 600_000 rounds), zeroized after use.

use std::fs;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{bail, Context, Result};
use dusk_bytes::Serializable;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use pbkdf2::pbkdf2_hmac;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroize;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const PBKDF2_ROUNDS: u32 = 600_000;

pub struct Identity {
    pub name: String,
    /// `None` for pk-only imports (foreign members).
    pub sk: Option<BlsSecretKey>,
    pub pk: BlsPublicKey,
}

impl Identity {
    pub fn is_pk_only(&self) -> bool {
        self.sk.is_none()
    }

    pub fn require_sk(&self) -> Result<&BlsSecretKey> {
        self.sk.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "identity '{}' is pk-only — cannot sign; import a full keypair or use a different signer",
                self.name
            )
        })
    }
}

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sk_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pk_hex: Option<String>,
}

pub fn default_path() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME must be set");
    Path::new(&home).join(".multisig-tool").join("identities.dat")
}

fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
    key
}

fn pk_from_hex(hex_s: &str) -> Result<BlsPublicKey> {
    let bytes = hex::decode(hex_s.trim_start_matches("0x")).context("pk_hex malformed")?;
    let arr: [u8; 96] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("pk_hex must be 96 bytes"))?;
    BlsPublicKey::from_bytes(&arr).map_err(|e| anyhow::anyhow!("invalid BlsPublicKey: {e:?}"))
}

fn pk_from_base58(s: &str) -> Result<BlsPublicKey> {
    let bytes = bs58::decode(s.trim())
        .into_vec()
        .context("pk base58 decode failed")?;
    let arr: [u8; 96] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("pk base58 must decode to 96 bytes"))?;
    BlsPublicKey::from_bytes(&arr).map_err(|e| anyhow::anyhow!("invalid BlsPublicKey: {e:?}"))
}

/// Accept hex (96 bytes) or base58-encoded compressed BLS public key.
pub fn parse_pk(s: &str) -> Result<BlsPublicKey> {
    let t = s.trim();
    if t.chars().all(|c| c.is_ascii_hexdigit()) || t.starts_with("0x") {
        pk_from_hex(t)
    } else {
        pk_from_base58(t)
    }
}

/// Loads and decrypts the store at `path`. Returns an empty list if the file
/// doesn't exist yet (fresh setup) — a wrong password against an *existing*
/// file fails decryption (AEAD tag mismatch), not silently returns garbage.
pub fn load(path: &Path, password: &str) -> Result<Vec<Identity>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    if raw.len() < SALT_LEN + NONCE_LEN {
        bail!("identity store file is too short / corrupt");
    }
    let (salt, rest) = raw.split_at(SALT_LEN);
    let (nonce_bytes, ciphertext) = rest.split_at(NONCE_LEN);

    let mut key_bytes = derive_key(password, salt);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| {
            anyhow::anyhow!(
                "wrong password, or identity store is corrupt (stores encrypted under the old \
                 100k-round KDF must be re-created — delete ~/.multisig-tool/identities.dat or \
                 your --store path and re-import identities)"
            )
        })?;
    key_bytes.zeroize();

    let stored: Vec<StoredIdentity> =
        serde_json::from_slice(&plaintext).context("identity store JSON is malformed")?;
    stored
        .into_iter()
        .map(|s| {
            if let Some(sk_hex) = s.sk_hex {
                let mut sk_bytes_vec = hex::decode(&sk_hex).context("identity sk_hex is malformed")?;
                let sk_bytes: [u8; 32] = sk_bytes_vec
                    .as_slice()
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("identity sk has wrong length"))?;
                sk_bytes_vec.zeroize();
                let sk = BlsSecretKey::from_bytes(&sk_bytes)
                    .map_err(|e| anyhow::anyhow!("identity sk bytes invalid: {e:?}"))?;
                let pk = BlsPublicKey::from(&sk);
                Ok(Identity {
                    name: s.name,
                    sk: Some(sk),
                    pk,
                })
            } else if let Some(pk_hex) = s.pk_hex {
                let pk = pk_from_hex(&pk_hex)?;
                Ok(Identity {
                    name: s.name,
                    sk: None,
                    pk,
                })
            } else {
                bail!("identity '{}' has neither sk_hex nor pk_hex", s.name)
            }
        })
        .collect()
}

/// Encrypts and saves `identities` to `path`, overwriting it. Fresh
/// salt/nonce every save (never reused).
pub fn save(path: &Path, password: &str, identities: &[Identity]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let stored: Vec<StoredIdentity> = identities
        .iter()
        .map(|id| match &id.sk {
            Some(sk) => StoredIdentity {
                name: id.name.clone(),
                sk_hex: Some(hex::encode(sk.to_bytes())),
                pk_hex: Some(hex::encode(id.pk.to_bytes())),
            },
            None => StoredIdentity {
                name: id.name.clone(),
                sk_hex: None,
                pk_hex: Some(hex::encode(id.pk.to_bytes())),
            },
        })
        .collect();
    let plaintext = serde_json::to_vec(&stored)?;

    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let mut key_bytes = derive_key(password, &salt);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;
    key_bytes.zeroize();

    let mut out = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    fs::write(path, &out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn generate(name: &str) -> Identity {
    let sk = BlsSecretKey::random(&mut OsRng);
    let pk = BlsPublicKey::from(&sk);
    Identity {
        name: name.to_string(),
        sk: Some(sk),
        pk,
    }
}

pub fn from_pk_only(name: &str, pk: BlsPublicKey) -> Identity {
    Identity {
        name: name.to_string(),
        sk: None,
        pk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_encrypt_decrypt() {
        let dir = std::env::temp_dir().join(format!(
            "multisig-keystore-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("identities.dat");
        let password = "test-password";
        let identity = generate("alice");
        save(&path, password, &[identity]).expect("save");
        let loaded = load(&path, password).expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "alice");
        assert!(!loaded[0].is_pk_only());
        std::fs::remove_dir_all(&dir).ok();
    }
}
