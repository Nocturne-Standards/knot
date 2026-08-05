//! Shared hex parsing helpers (single `0x` strip, reject `0x0x`).

use anyhow::{Context, Result, bail};

/// Strip one optional `0x` prefix; reject repeated `0x`.
pub fn strip_single_0x(s: &str) -> Result<&str> {
    let t = s.trim();
    match t.strip_prefix("0x") {
        None => Ok(t),
        Some(rest) if rest.starts_with("0x") => bail!("repeated 0x prefix"),
        Some(rest) => Ok(rest),
    }
}

/// Decode hex bytes after [`strip_single_0x`].
pub fn decode_hex(s: &str, label: &str) -> Result<Vec<u8>> {
    let stripped = strip_single_0x(s).with_context(|| format!("{label} malformed"))?;
    hex::decode(stripped).with_context(|| format!("{label} hex"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_rejects_double_prefix() {
        assert!(strip_single_0x("0x0xab").is_err());
        assert_eq!(strip_single_0x("0xab").unwrap(), "ab");
        assert_eq!(strip_single_0x("ab").unwrap(), "ab");
    }
}
