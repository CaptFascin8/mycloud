//! MyCloud — `auth` canister
//!
//! Two responsibilities:
//!   1. User registry: bind a caller's Internet Identity Principal to a
//!      User record stored in stable memory (survives upgrades).
//!   2. Credential vault: per-user labeled blobs, access scoped to the
//!      owning principal.
//!
//! Plus a `health_check()` endpoint the manager canister will poll.
//!
//! NOT YET: encryption. Blobs are stored as-is, visible to subnet node
//! operators. Real protection is client-side encryption before storing —
//! a frontend concern, planned for Checkpoint 3c.

use candid::{CandidType, Decode, Encode, Principal};
use ic_cdk::{init, query, update};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, Storable};
use ic_stable_structures::storable::Bound;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::RefCell;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A registered MyCloud user. Bound to one Internet Identity principal.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct User {
    pub id:         Principal,
    pub registered: u64,    // ic_cdk::api::time() — ns since epoch
    pub last_seen:  u64,    // updated on every authenticated call
}

/// One credential entry in a user's vault. The blob is opaque bytes — the
/// canister never inspects it. Clients are expected to encrypt before
/// storing.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct CredentialEntry {
    pub label:        String,                // e.g. "github_pat", "openai_key"
    pub data:         serde_bytes::ByteBuf,  // opaque; client-encrypted
    pub created_ns:   u64,
    pub updated_ns:   u64,
}

/// Composite key for the credential vault. Stored as serialized bytes so
/// `StableBTreeMap` can use it directly.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct VaultKey {
    owner: Principal,
    label: String,
}

/// Health status reported back to the manager canister.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct HealthStatus {
    pub canister:      String,    // "auth"
    pub ok:            bool,
    pub user_count:    u64,
    pub vault_entries: u64,
    pub timestamp_ns:  u64,
}

/// Errors returned to clients. Specific enough to act on, vague enough not
/// to leak internals.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum AuthError {
    NotRegistered,
    NotFound,
    Unauthorized,
    BadInput(String),
}

// ---------------------------------------------------------------------------
// Storable impls — required for stable memory storage
// ---------------------------------------------------------------------------

impl Storable for User {
    fn to_bytes(&self) -> Cow<[u8]> { Cow::Owned(Encode!(self).unwrap()) }
    fn from_bytes(b: Cow<[u8]>) -> Self { Decode!(&b, User).unwrap() }
    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for CredentialEntry {
    fn to_bytes(&self) -> Cow<[u8]> { Cow::Owned(Encode!(self).unwrap()) }
    fn from_bytes(b: Cow<[u8]>) -> Self { Decode!(&b, CredentialEntry).unwrap() }
    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for VaultKey {
    fn to_bytes(&self) -> Cow<[u8]> {
        // Encode as (Principal as bytes, label as utf8). Fixed-size principal
        // first so ordering is principal-then-label — efficient owner range
        // scans later.
        let p = self.owner.as_slice();
        let mut out = Vec::with_capacity(1 + p.len() + 1 + self.label.len());
        out.push(p.len() as u8);
        out.extend_from_slice(p);
        out.extend_from_slice(self.label.as_bytes());
        Cow::Owned(out)
    }
    fn from_bytes(b: Cow<[u8]>) -> Self {
        let plen = b[0] as usize;
        let owner = Principal::from_slice(&b[1..1 + plen]);
        let label = String::from_utf8(b[1 + plen..].to_vec()).unwrap();
        VaultKey { owner, label }
    }
    const BOUND: Bound = Bound::Bounded { max_size: 256, is_fixed_size: false };
}

// ---------------------------------------------------------------------------
// Stable memory layout
// ---------------------------------------------------------------------------

type Memory = VirtualMemory<DefaultMemoryImpl>;

const MEM_USERS:        MemoryId = MemoryId::new(0);
const MEM_VAULT:        MemoryId = MemoryId::new(1);

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static USERS: RefCell<StableBTreeMap<Principal, User, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MEM_USERS))
        )
    );

    static VAULT: RefCell<StableBTreeMap<VaultKey, CredentialEntry, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MEM_VAULT))
        )
    );
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

#[init]
fn init() {
    ic_cdk::println!("auth canister initialized");
}

// post_upgrade is a no-op: ic-stable-structures handles persistence
// automatically across upgrades. We just have to NOT clobber the cells.
#[ic_cdk::post_upgrade]
fn post_upgrade() {
    ic_cdk::println!("auth canister upgraded; stable storage preserved");
}

// ---------------------------------------------------------------------------
// Public API — user registry
// ---------------------------------------------------------------------------

/// Echo the caller's principal. Smoke test for Internet Identity wiring.
#[query]
fn whoami() -> Principal {
    ic_cdk::api::caller()
}

/// Register the caller (or refresh `last_seen` if already registered).
/// Returns the User record either way — idempotent on purpose.
#[update]
fn register() -> User {
    let caller = ic_cdk::api::caller();
    if caller == Principal::anonymous() {
        ic_cdk::trap("anonymous principals cannot register");
    }
    let now = ic_cdk::api::time();

    USERS.with(|users| {
        let mut users = users.borrow_mut();
        let user = match users.get(&caller) {
            Some(mut u) => {
                u.last_seen = now;
                users.insert(caller, u.clone());
                u
            }
            None => {
                let u = User { id: caller, registered: now, last_seen: now };
                users.insert(caller, u.clone());
                u
            }
        };
        user
    })
}

/// Look up the caller's own User record without mutating last_seen.
#[query]
fn get_me() -> Result<User, AuthError> {
    let caller = ic_cdk::api::caller();
    USERS.with(|users| users.borrow().get(&caller).ok_or(AuthError::NotRegistered))
}

/// Total registered users. Used by the manager's health check + the dashboard.
#[query]
fn user_count() -> u64 {
    USERS.with(|users| users.borrow().len())
}

// ---------------------------------------------------------------------------
// Public API — credential vault (caller-scoped)
// ---------------------------------------------------------------------------

/// Store or update a credential under the caller's vault.
#[update]
fn put_credential(label: String, data: serde_bytes::ByteBuf) -> Result<CredentialEntry, AuthError> {
    let caller = ic_cdk::api::caller();
    require_registered(caller)?;
    if label.is_empty() || label.len() > 128 {
        return Err(AuthError::BadInput("label must be 1..=128 chars".into()));
    }
    if data.len() > 64 * 1024 {
        return Err(AuthError::BadInput("blob exceeds 64KiB limit".into()));
    }
    let now = ic_cdk::api::time();
    let key = VaultKey { owner: caller, label: label.clone() };
    VAULT.with(|v| {
        let mut v = v.borrow_mut();
        let entry = match v.get(&key) {
            Some(existing) => CredentialEntry {
                label,
                data,
                created_ns: existing.created_ns,
                updated_ns: now,
            },
            None => CredentialEntry {
                label,
                data,
                created_ns: now,
                updated_ns: now,
            },
        };
        v.insert(key, entry.clone());
        Ok(entry)
    })
}

/// Fetch one credential by label. Caller must own it.
#[query]
fn get_credential(label: String) -> Result<CredentialEntry, AuthError> {
    let caller = ic_cdk::api::caller();
    require_registered(caller)?;
    let key = VaultKey { owner: caller, label };
    VAULT.with(|v| v.borrow().get(&key).ok_or(AuthError::NotFound))
}

/// List all of the caller's credential labels (NOT the blobs — that'd be
/// a needless replicated read of every entry).
#[query]
fn list_credentials() -> Result<Vec<String>, AuthError> {
    let caller = ic_cdk::api::caller();
    require_registered(caller)?;
    VAULT.with(|v| {
        let v = v.borrow();
        let labels = v
            .iter()
            .filter(|(k, _)| k.owner == caller)
            .map(|(k, _)| k.label.clone())
            .collect();
        Ok(labels)
    })
}

/// Delete one of the caller's credentials.
#[update]
fn delete_credential(label: String) -> Result<bool, AuthError> {
    let caller = ic_cdk::api::caller();
    require_registered(caller)?;
    let key = VaultKey { owner: caller, label };
    VAULT.with(|v| Ok(v.borrow_mut().remove(&key).is_some()))
}

// ---------------------------------------------------------------------------
// Public API — health (polled by manager)
// ---------------------------------------------------------------------------

/// Health snapshot. Manager polls this and treats `ok == true` as alive.
#[query]
fn health_check() -> HealthStatus {
    HealthStatus {
        canister:      "auth".to_string(),
        ok:            true,
        user_count:    USERS.with(|u| u.borrow().len()),
        vault_entries: VAULT.with(|v| v.borrow().len()),
        timestamp_ns:  ic_cdk::api::time(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_registered(caller: Principal) -> Result<(), AuthError> {
    if caller == Principal::anonymous() {
        return Err(AuthError::Unauthorized);
    }
    USERS.with(|u| {
        if u.borrow().contains_key(&caller) {
            Ok(())
        } else {
            Err(AuthError::NotRegistered)
        }
    })
}

// ---------------------------------------------------------------------------
// Candid export
// ---------------------------------------------------------------------------

ic_cdk::export_candid!();

// ---------------------------------------------------------------------------
// Unit tests — pure logic only. Anything touching `ic_cdk::api::caller()`
// or stable memory needs `dfx canister call` (see scripts/test_auth.sh).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_key_roundtrip() {
        let original = VaultKey {
            owner: Principal::from_slice(&[1, 2, 3, 4]),
            label: "github_pat".to_string(),
        };
        let bytes = original.to_bytes();
        let decoded = VaultKey::from_bytes(bytes);
        assert_eq!(original.owner, decoded.owner);
        assert_eq!(original.label, decoded.label);
    }

    #[test]
    fn vault_key_ordering_groups_by_owner() {
        // Same owner, different labels — should sort together.
        let alice_a = VaultKey { owner: Principal::from_slice(&[1]), label: "a".into() };
        let alice_b = VaultKey { owner: Principal::from_slice(&[1]), label: "b".into() };
        let bob_a   = VaultKey { owner: Principal::from_slice(&[2]), label: "a".into() };

        let mut keys = vec![bob_a.clone(), alice_b.clone(), alice_a.clone()];
        keys.sort();
        assert_eq!(keys, vec![alice_a, alice_b, bob_a]);
    }
}
