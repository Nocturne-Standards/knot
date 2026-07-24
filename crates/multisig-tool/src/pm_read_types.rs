//! Minimal prediction-market free-read types for RUES decoding.
//!
//! Kept in sync by hand with
//! `prediction-market/crates/prediction-market/src/call_types.rs`
//! (`MarketStatus` / `MarketInfo` only). Do not expand this file with write
//! arg types — path-include the real module if that becomes necessary.

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[repr(u8)]
pub enum MarketStatus {
    Open = 0,
    PendingYes = 1,
    PendingNo = 2,
    UnderReview = 3,
    Finalized = 4,
    Cancelled = 5,
}

impl MarketStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::PendingYes => "PendingYes",
            Self::PendingNo => "PendingNo",
            Self::UnderReview => "UnderReview",
            Self::Finalized => "Finalized",
            Self::Cancelled => "Cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub struct MarketInfo {
    pub question_hash: [u8; 32],
    pub content_cid: [u8; 32],
    pub close_block: u64,
    pub challenge_window_blocks: u64,
    pub review_window_blocks: u64,
    pub yes_reserve: u64,
    pub no_reserve: u64,
    pub min_bond: u64,
    pub status: MarketStatus,
    pub winning_outcome: Option<u8>,
    pub fee_bps: u16,
    pub lp_fee_pot: u64,
}
