// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nocturne Standards

//! Proposal deadlines are `now + min(proposal_ttl, SENSIBLE_PROPOSAL_WINDOW)`.
//! A caller-supplied deadline is kept only when it sits inside that chain window.

/// Blocks. About fifteen minutes on a 10-second testnet block.
pub const SENSIBLE_PROPOSAL_WINDOW: u64 = 100;

/// Approximate seconds per block, for the Lab preview only.
pub const APPROX_BLOCK_SECONDS: u64 = 10;

pub fn default_proposal_deadline(now: u64, ttl: u64) -> Option<u64> {
    if ttl == 0 {
        return None;
    }
    now.checked_add(ttl.min(SENSIBLE_PROPOSAL_WINDOW))
}

/// `0`, a past block, or anything past `now + ttl` becomes the default.
pub fn resolve_proposal_deadline(requested: u64, now: u64, ttl: u64) -> Option<u64> {
    let max = now.checked_add(ttl)?;
    if requested == 0 || requested < now || requested > max {
        return default_proposal_deadline(now, ttl);
    }
    Some(requested)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_ttl_caps_the_window() {
        assert_eq!(default_proposal_deadline(10, 40), Some(50));
        assert_eq!(resolve_proposal_deadline(0, 10, 40), Some(50));
        assert_eq!(resolve_proposal_deadline(9_999, 10, 40), Some(50));
    }

    #[test]
    fn explicit_deadline_inside_the_window_is_kept() {
        assert_eq!(resolve_proposal_deadline(30, 10, 1_000), Some(30));
    }

    #[test]
    fn large_ttl_uses_the_sensible_window() {
        assert_eq!(default_proposal_deadline(1_000, 1_000), Some(1_100));
    }
}
