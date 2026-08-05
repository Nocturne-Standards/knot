//! Encrypted local store of named BLS "member" identities — **test tooling only**.
//! Not production key custody: in production each identity holds its own wallet key.
//!
//! v2 file layout (§3.3):
//! `[magic:8][version:1][kdf_id:1][kdf_p1:4][kdf_p2:4][kdf_p3:1][salt_len:1][salt:N][nonce:12][AES-256-GCM ct]`
//! Header bytes are AES-GCM associated data. Plaintext is fixed-layout binary (no JSON).
//!
//! v1 (legacy): `[salt:16][nonce:12][AES-256-GCM ct]` with JSON plaintext, PBKDF2 600k.
//! v1 files load and are silently re-saved as v2.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{bail, Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use dusk_bytes::Serializable;
use dusk_core::signatures::bls::{PublicKey as BlsPublicKey, SecretKey as BlsSecretKey};
use pbkdf2::pbkdf2_hmac;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::{Zeroize, Zeroizing};

const MAGIC: &[u8; 8] = b"KNOTKS\x00\x02";
const FORMAT_VERSION: u8 = 2;
const KDF_PBKDF2: u8 = 1;
const KDF_ARGON2ID: u8 = 2;
const V1_SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const PBKDF2_ROUNDS: u32 = 600_000;
const ARGON2_M_COST_KIB: u32 = 65_536; // 64 MiB
const ARGON2_T_COST: u32 = 3;
const ARGON2_P_COST: u8 = 4;
const DEFAULT_SALT_LEN: usize = 16;

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

/// Default store path. Honors `KNOT_STORE`; otherwise uses platform data dir via `directories`.
pub fn default_path() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("KNOT_STORE") {
        return Ok(PathBuf::from(p));
    }
    let dirs = directories::ProjectDirs::from("", "", "knot")
        .ok_or_else(|| anyhow::anyhow!("cannot resolve platform data directory"))?;
    Ok(dirs.data_dir().join("identities.dat"))
}

/// Resolve store path, falling back to legacy locations for one release when the new path is absent.
pub fn resolve_default_path() -> Result<PathBuf> {
    let primary = default_path()?;
    if primary.exists() {
        return Ok(primary);
    }
    if let Ok(home) = std::env::var("HOME") {
        for legacy_dir in [".knot", ".knot-tool", ".multisig-tool"] {
            let cand = Path::new(&home).join(legacy_dir).join("identities.dat");
            if cand.exists() {
                eprintln!(
                    "warning: using legacy keystore at {}; prefer {}",
                    cand.display(),
                    primary.display()
                );
                return Ok(cand);
            }
        }
    }
    Ok(primary)
}

#[cfg(target_os = "macos")]
fn full_fsync(f: &File) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    if unsafe { libc::fcntl(f.as_raw_fd(), libc::F_FULLFSYNC) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn full_fsync(_: &File) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn check_store_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)?.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        bail!("identity store is group/world accessible (mode {mode:o}) — chmod 600");
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_store_permissions(_: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn ensure_parent_dir(parent: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .with_context(|| format!("creating {}", parent.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn ensure_parent_dir(parent: &Path) -> Result<()> {
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    fs::create_dir_all(parent)
        .with_context(|| format!("creating {}", parent.display()))?;
    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    path.with_extension("tmp")
}

fn bak_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".bak");
    PathBuf::from(s)
}

fn cleanup_stale_tmp(path: &Path) -> Result<()> {
    let tmp = tmp_path(path);
    if tmp.exists() {
        fs::remove_file(&tmp)
            .with_context(|| format!("removing stale tmp {}", tmp.display()))?;
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .context("store path has no parent")?;
    ensure_parent_dir(dir)?;
    let tmp = tmp_path(path);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .with_context(|| format!("opening tmp {}", tmp.display()))?;
        f.write_all(bytes)?;
        f.sync_all()?;
        full_fsync(&f)?;
        drop(f);
    }
    #[cfg(not(unix))]
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)
            .with_context(|| format!("opening tmp {}", tmp.display()))?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
    }

    fs::rename(&tmp, path)?;

    let dfd = File::open(dir)?;
    dfd.sync_all()?;
    full_fsync(&dfd)?;
    Ok(())
}

fn rotate_backup(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let bak = bak_path(path);
    if bak.exists() {
        fs::remove_file(&bak)?;
    }
    fs::rename(path, &bak).with_context(|| {
        format!(
            "rotating backup {} -> {}",
            path.display(),
            bak.display()
        )
    })?;
    Ok(())
}

struct KdfHeader {
    kdf_id: u8,
    p1: u32,
    p2: u32,
    p3: u8,
    salt: Vec<u8>,
}

impl KdfHeader {
    fn new_argon2id(salt: Vec<u8>) -> Self {
        Self {
            kdf_id: KDF_ARGON2ID,
            p1: ARGON2_M_COST_KIB,
            p2: ARGON2_T_COST,
            p3: ARGON2_P_COST,
            salt,
        }
    }

    fn new_pbkdf2(salt: Vec<u8>) -> Self {
        Self {
            kdf_id: KDF_PBKDF2,
            p1: PBKDF2_ROUNDS,
            p2: 0,
            p3: 0,
            salt,
        }
    }

    fn encode_prefix(&self) -> Vec<u8> {
        let mut h = Vec::with_capacity(20 + self.salt.len());
        h.extend_from_slice(MAGIC);
        h.push(FORMAT_VERSION);
        h.push(self.kdf_id);
        h.extend_from_slice(&self.p1.to_le_bytes());
        h.extend_from_slice(&self.p2.to_le_bytes());
        h.push(self.p3);
        h.push(
            self.salt
                .len()
                .try_into()
                .expect("salt length fits in u8"),
        );
        h.extend_from_slice(&self.salt);
        h
    }

    fn parse(raw: &[u8]) -> Result<(Self, usize)> {
        if raw.len() < 20 {
            bail!("identity store file is too short / corrupt");
        }
        if &raw[..8] != MAGIC {
            bail!("identity store has invalid magic");
        }
        if raw[8] != FORMAT_VERSION {
            bail!("unsupported identity store version {}", raw[8]);
        }
        let kdf_id = raw[9];
        let p1 = u32::from_le_bytes(raw[10..14].try_into()?);
        let p2 = u32::from_le_bytes(raw[14..18].try_into()?);
        let p3 = raw[18];
        let salt_len = raw[19] as usize;
        let header_end = 20 + salt_len;
        if raw.len() < header_end + NONCE_LEN {
            bail!("identity store file is too short / corrupt");
        }
        let salt = raw[20..header_end].to_vec();
        Ok((
            Self {
                kdf_id,
                p1,
                p2,
                p3,
                salt,
            },
            header_end,
        ))
    }
}

fn derive_key(header: &KdfHeader, password: &str) -> Result<Zeroizing<[u8; 32]>> {
    let mut key = Zeroizing::new([0u8; 32]);
    match header.kdf_id {
        KDF_PBKDF2 => {
            pbkdf2_hmac::<Sha256>(
                password.as_bytes(),
                &header.salt,
                header.p1,
                key.as_mut(),
            );
        }
        KDF_ARGON2ID => {
            let params = Params::new(header.p1, header.p2, u32::from(header.p3), Some(32))
                .map_err(|e| anyhow::anyhow!("argon2 params: {e}"))?;
            let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
            argon2
                .hash_password_into(password.as_bytes(), &header.salt, key.as_mut())
                .map_err(|e| anyhow::anyhow!("argon2 key derivation failed: {e}"))?;
        }
        other => bail!("unsupported kdf_id {other}"),
    }
    Ok(key)
}

fn encode_plaintext(identities: &[Identity]) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(identities.len() as u32).to_le_bytes());
    for id in identities {
        let name_bytes = id.name.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            bail!("identity name '{}' is too long", id.name);
        }
        buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(name_bytes);
        match &id.sk {
            Some(sk) => {
                buf.push(1);
                buf.extend_from_slice(&sk.to_bytes());
            }
            None => buf.push(0),
        }
        buf.extend_from_slice(&id.pk.to_bytes());
    }
    Ok(buf)
}

fn decode_plaintext(plaintext: &[u8]) -> Result<Vec<Identity>> {
    if plaintext.len() < 4 {
        bail!("identity store plaintext is corrupt");
    }
    let count = u32::from_le_bytes(plaintext[..4].try_into()?) as usize;
    let mut offset = 4;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if offset + 2 > plaintext.len() {
            bail!("identity store plaintext is corrupt");
        }
        let name_len = u16::from_le_bytes(plaintext[offset..offset + 2].try_into()?) as usize;
        offset += 2;
        if offset + name_len + 1 + 96 > plaintext.len() {
            bail!("identity store plaintext is corrupt");
        }
        let name = std::str::from_utf8(&plaintext[offset..offset + name_len])
            .context("identity name is not valid UTF-8")?
            .to_string();
        offset += name_len;
        let has_sk = plaintext[offset];
        offset += 1;
        let sk = if has_sk == 1 {
            if offset + 32 > plaintext.len() {
                bail!("identity store plaintext is corrupt");
            }
            let sk_bytes: [u8; 32] = plaintext[offset..offset + 32].try_into()?;
            offset += 32;
            let sk = BlsSecretKey::from_bytes(&sk_bytes)
                .map_err(|e| anyhow::anyhow!("identity sk bytes invalid: {e:?}"))?;
            Some(sk)
        } else if has_sk == 0 {
            None
        } else {
            bail!("identity store plaintext is corrupt");
        };
        let pk_bytes: [u8; 96] = plaintext[offset..offset + 96].try_into()?;
        offset += 96;
        let pk = BlsPublicKey::from_bytes(&pk_bytes)
            .map_err(|e| anyhow::anyhow!("invalid BlsPublicKey: {e:?}"))?;
        out.push(Identity { name, sk, pk });
    }
    if offset != plaintext.len() {
        bail!("identity store plaintext is corrupt");
    }
    Ok(out)
}

fn decrypt_v2(raw: &[u8], password: &str) -> Result<Vec<Identity>> {
    let (header, salt_end) = KdfHeader::parse(raw)?;
    let aad = &raw[..salt_end];
    let nonce_bytes = &raw[salt_end..salt_end + NONCE_LEN];
    let ciphertext = &raw[salt_end + NONCE_LEN..];

    let key = derive_key(&header, password)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_ref()));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("decryption failed (wrong password or corrupt store)"))?,
    );
    decode_plaintext(&plaintext)
}

#[derive(Serialize, Deserialize)]
struct StoredIdentityV1 {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sk_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pk_hex: Option<String>,
}

fn decrypt_v1(raw: &[u8], password: &str) -> Result<Vec<Identity>> {
    if raw.len() < V1_SALT_LEN + NONCE_LEN {
        bail!("identity store file is too short / corrupt");
    }
    let (salt, rest) = raw.split_at(V1_SALT_LEN);
    let (nonce_bytes, ciphertext) = rest.split_at(NONCE_LEN);
    let header = KdfHeader::new_pbkdf2(salt.to_vec());
    let key = derive_key(&header, password)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_ref()));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("wrong password, or identity store is corrupt"))?;
    let stored: Vec<StoredIdentityV1> =
        serde_json::from_slice(&plaintext).context("identity store JSON is malformed")?;
    stored
        .into_iter()
        .map(|s| {
            if let Some(sk_hex) = s.sk_hex {
                let stripped = strip_single_0x(&sk_hex).context("identity sk_hex malformed")?;
                let mut sk_bytes_vec = hex::decode(stripped).context("identity sk_hex is malformed")?;
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

fn is_v2(raw: &[u8]) -> bool {
    raw.len() >= MAGIC.len() && &raw[..MAGIC.len()] == MAGIC
}

fn encrypt_v2(password: &str, identities: &[Identity]) -> Result<Vec<u8>> {
    let plaintext = encode_plaintext(identities)?;
    let mut salt = vec![0u8; DEFAULT_SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let header = KdfHeader::new_argon2id(salt);
    let aad = header.encode_prefix();
    let key = derive_key(&header, password)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_ref()));
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: &plaintext,
                aad: &aad,
            },
        )
        .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

    let mut out = aad;
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

fn strip_single_0x(s: &str) -> Result<&str> {
    let t = s.trim();
    match t.strip_prefix("0x") {
        None => Ok(t),
        Some(rest) if rest.starts_with("0x") => {
            bail!("repeated 0x prefix")
        }
        Some(rest) => Ok(rest),
    }
}

fn pk_from_hex(hex_s: &str) -> Result<BlsPublicKey> {
    let stripped = strip_single_0x(hex_s).context("pk_hex malformed")?;
    let bytes = hex::decode(stripped).context("pk_hex malformed")?;
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
    let hex_err = match pk_from_hex(t) {
        Ok(pk) => return Ok(pk),
        Err(e) => e,
    };
    let b58_err = match pk_from_base58(t) {
        Ok(pk) => return Ok(pk),
        Err(e) => e,
    };
    bail!(
        "not a valid BLS public key.\n  as hex:    {hex_err}\n  as base58: {b58_err}"
    )
}

/// Loads and decrypts the store at `path`. Returns an empty list if the file
/// doesn't exist yet. v1 stores are silently upgraded to v2 on load.
pub fn load(path: &Path, password: &str) -> Result<Vec<Identity>> {
    cleanup_stale_tmp(path)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    check_store_permissions(path)?;
    let raw = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let (identities, was_v1) = if is_v2(&raw) {
        (decrypt_v2(&raw, password)?, false)
    } else {
        (decrypt_v1(&raw, password)?, true)
    };
    if was_v1 {
        save(path, password, &identities)?;
    }
    Ok(identities)
}

/// Encrypts and saves `identities` to `path` (v2 format, atomic write, `.bak` rotation).
pub fn save(path: &Path, password: &str, identities: &[Identity]) -> Result<()> {
    let bytes = encrypt_v2(password, identities)?;
    rotate_backup(path)?;
    write_atomic(path, &bytes)?;
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

    fn test_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "knot-keystore-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn round_trip_encrypt_decrypt() {
        let dir = test_dir("roundtrip");
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

    #[cfg(unix)]
    #[test]
    fn save_sets_mode_600_and_parent_700() {
        use std::os::unix::fs::PermissionsExt;
        let dir = test_dir("perms");
        let path = dir.join("nested").join("identities.dat");
        save(&path, "pw", &[generate("a")]).expect("save");
        let file_mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let parent_mode = fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600, "file mode");
        assert_eq!(parent_mode, 0o700, "parent mode");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn load_refuses_over_permissive_store() {
        use std::os::unix::fs::PermissionsExt;
        let dir = test_dir("chmod");
        let path = dir.join("identities.dat");
        save(&path, "pw", &[generate("a")]).expect("save");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let err = load(&path, "pw").err().expect("should fail").to_string();
        assert!(
            err.contains("group/world accessible"),
            "unexpected: {err}"
        );
        assert!(err.contains("chmod 600"), "unexpected: {err}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stale_tmp_is_cleaned_on_load() {
        let dir = test_dir("staletmp");
        let path = dir.join("identities.dat");
        save(&path, "pw", &[generate("a")]).expect("save");
        let tmp = tmp_path(&path);
        fs::write(&tmp, b"stale").unwrap();
        assert!(tmp.exists());
        let loaded = load(&path, "pw").expect("load");
        assert_eq!(loaded.len(), 1);
        assert!(!tmp.exists(), "stale tmp should be removed");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn interrupted_write_leaves_old_store_loadable() {
        let dir = test_dir("atomic");
        let path = dir.join("identities.dat");
        save(&path, "pw", &[generate("alice")]).expect("save");
        let before = fs::read(&path).unwrap();
        let tmp = tmp_path(&path);
        fs::write(&tmp, b"partial-write-not-renamed").unwrap();
        let loaded = load(&path, "pw").expect("old store still loads");
        assert_eq!(loaded[0].name, "alice");
        let after = fs::read(&path).unwrap();
        assert_eq!(before, after);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn v1_load_silently_upgrades_to_v2() {
        let dir = test_dir("v1upgrade");
        let path = dir.join("identities.dat");
        let password = "pw";
        let identity = generate("bob");

        // Write a v1 file manually.
        let stored = vec![StoredIdentityV1 {
            name: identity.name.clone(),
            sk_hex: Some(hex::encode(identity.sk.as_ref().unwrap().to_bytes())),
            pk_hex: Some(hex::encode(identity.pk.to_bytes())),
        }];
        let plaintext = serde_json::to_vec(&stored).unwrap();
        let mut salt = [0u8; V1_SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let header = KdfHeader::new_pbkdf2(salt.to_vec());
        let key = derive_key(&header, password).unwrap();
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_ref()));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_ref())
            .unwrap();
        let mut v1 = Vec::new();
        v1.extend_from_slice(&salt);
        v1.extend_from_slice(&nonce_bytes);
        v1.extend_from_slice(&ciphertext);
        fs::write(&path, &v1).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }

        assert!(!is_v2(&v1));
        let loaded = load(&path, password).expect("v1 load");
        assert_eq!(loaded.len(), 1);
        let on_disk = fs::read(&path).unwrap();
        assert!(is_v2(&on_disk), "should upgrade to v2 on disk");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn v2_tampered_header_fails_aead() {
        let dir = test_dir("aead");
        let path = dir.join("identities.dat");
        let password = "pw";
        save(&path, password, &[generate("x")]).expect("save");
        let mut raw = fs::read(&path).unwrap();
        raw[10] ^= 0xff;
        fs::write(&path, &raw).unwrap();
        let err = load(&path, password).err().expect("should fail").to_string();
        assert!(
            err.contains("decryption failed"),
            "expected AEAD failure, got: {err}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_pk_hex_and_base58_same_key() {
        let id = generate("k");
        let hex_pk = hex::encode(id.pk.to_bytes());
        let b58_pk = bs58::encode(id.pk.to_bytes()).into_string();
        let from_hex = parse_pk(&hex_pk).expect("hex");
        let from_b58 = parse_pk(&b58_pk).expect("b58");
        assert_eq!(from_hex.to_bytes(), from_b58.to_bytes());
    }

    #[test]
    fn parse_pk_rejects_double_0x_prefix() {
        let id = generate("k");
        let hex_pk = format!("0x0x{}", hex::encode(id.pk.to_bytes()));
        assert!(parse_pk(&hex_pk).is_err());
    }

    #[test]
    fn default_path_without_home_does_not_panic() {
        let keys = ["HOME", "KNOT_STORE", "XDG_DATA_HOME", "USERPROFILE"];
        let saved: Vec<(&str, String)> = keys
            .iter()
            .filter_map(|k| std::env::var(k).ok().map(|v| (*k, v)))
            .collect();
        for k in &keys {
            // SAFETY: test-only env mutation; single-threaded test harness.
            unsafe { std::env::remove_var(k) };
        }
        let _ = default_path();
        for (k, v) in saved {
            unsafe { std::env::set_var(k, v) };
        }
    }

    #[test]
    fn knot_store_override() {
        let saved = std::env::var("KNOT_STORE").ok();
        unsafe { std::env::set_var("KNOT_STORE", "/tmp/custom-identities.dat") };
        let p = default_path().expect("KNOT_STORE");
        assert_eq!(p, PathBuf::from("/tmp/custom-identities.dat"));
        if let Some(s) = saved {
            unsafe { std::env::set_var("KNOT_STORE", s) };
        } else {
            unsafe { std::env::remove_var("KNOT_STORE") };
        }
    }
}
