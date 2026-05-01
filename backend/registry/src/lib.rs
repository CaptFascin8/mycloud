//! MyCloud — `registry` canister
//!
//! Tracks "smartsites": named sites whose ownership is provable and whose
//! content lives on IPFS, addressed by CID.
//!
//! Cross-project alignment:
//!   * Crystal Dragon Yggdrasil KEY tiers (ROOT/TRUNK/BRANCH/CROWN/DOMAIN)
//!     are first-class via `OwnershipProof::SolanaNft { tier: Option<KeyTier> }`.
//!   * Agentic Acres can register agent-home sites so nomadic Sally has a
//!     queryable "current address" from any client.

use candid::{CandidType, Principal};
use ic_cdk::{init, query, update};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct Smartsite {
    pub domain:     String,
    pub owner:      Principal,
    pub ipfs_cid:   String,
    pub created_ns: u64,
    pub updated_ns: u64,
    pub ownership:  OwnershipProof,
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum OwnershipProof {
    InternetIdentity,
    SolanaNft   { mint: String, wallet: String, tier: Option<KeyTier> },
    EthereumNft { contract: String, token_id: String, wallet: String },
}

/// Crystal Dragon Yggdrasil KEY tier system per MASTER_CHECKLIST.md:
///   ROOT   #0001-0100   (Genesis)
///   TRUNK  #0101-1000   (Standard)
///   BRANCH #1001-5000   (Premium)
///   CROWN  #5001-10000  (Elite)
///   DOMAIN unlimited    (Transfer KEY)
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, Copy)]
pub enum KeyTier { Root, Trunk, Branch, Crown, Domain }

#[init]
fn init() {
    ic_cdk::println!("registry canister initialized");
}

#[query]
fn list_sites() -> Vec<Smartsite> {
    Vec::new()
}

#[update]
fn register_site(domain: String, ipfs_cid: String) -> Smartsite {
    let now = ic_cdk::api::time();
    Smartsite {
        domain,
        owner:      ic_cdk::api::caller(),
        ipfs_cid,
        created_ns: now,
        updated_ns: now,
        ownership:  OwnershipProof::InternetIdentity,
    }
}

ic_cdk::export_candid!();
