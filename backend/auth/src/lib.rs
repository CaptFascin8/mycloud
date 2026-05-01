//! MyCloud — `auth` canister
//!
//! Binds the caller's Internet Identity Principal to a user record and
//! stores per-user credentials in stable memory.
//!
//! Checkpoint 2 deliverable: compiles + .did is consistent. Real logic
//! (stable storage, credential vault) lands in Checkpoint 3.

use candid::{CandidType, Principal};
use ic_cdk::{init, query, update};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct User {
    pub principal:  Principal,
    pub registered: u64, // ic_cdk::api::time() — ns since epoch
}

#[init]
fn init() {
    ic_cdk::println!("auth canister initialized");
}

#[query]
fn whoami() -> Principal {
    ic_cdk::api::caller()
}

#[update]
fn register() -> User {
    User {
        principal:  ic_cdk::api::caller(),
        registered: ic_cdk::api::time(),
    }
}

ic_cdk::export_candid!();
