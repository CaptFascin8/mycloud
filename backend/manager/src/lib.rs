//! MyCloud — `manager` canister ("Smart Agent")
//!
//! Periodic ic-cdk-timers job that samples the canister's own cycle balance,
//! calls auth+registry for liveness, and keeps a ring buffer of recent
//! HealthEvents queryable by the dashboard.
//!
//! Checkpoint 2: compiles + .did consistent. Timer wiring + ring buffer
//! land in Checkpoint 3.

use candid::CandidType;
use ic_cdk::{init, query};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum Severity { Info, Warn, Error }

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct HealthEvent {
    pub timestamp_ns: u64,
    pub source:       String,
    pub severity:     Severity,
    pub message:      String,
}

#[init]
fn init() {
    ic_cdk::println!("manager canister initialized");
}

#[query]
fn recent_events(_limit: u32) -> Vec<HealthEvent> {
    Vec::new()
}

#[query]
fn cycles_balance() -> u64 {
    ic_cdk::api::canister_balance()
}

ic_cdk::export_candid!();
