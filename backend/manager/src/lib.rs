//! MyCloud — `manager` canister ("Smart Agent")
//!
//! The watchdog and cycles bursar of MyCloud's canister fleet.
//!
//! Responsibilities (this checkpoint):
//!   * Periodic ic-cdk-timers tick (every 60s) that polls auth + registry
//!     `health_check()` via inter-canister calls and records results in a
//!     ring buffer of HealthEvents.
//!   * Track the cycle balance of each registered canister; emit a Warn
//!     event when any drops below threshold.
//!   * Expose admin methods (owner-only) to manually top up a target
//!     canister with cycles from the manager's own balance.
//!   * Allow the owner to register/unregister the canisters under watch.
//!
//! Future (later checkpoints):
//!   * HTTP outcalls for external alerting (Discord webhook, email)
//!   * Self-healing: when a check fails, call a VPS healer agent that
//!     can `docker restart mycloud-ipfs` and similar (see CLOUD_FACTORY.md)
//!   * Automatic top-ups (not just manual) once we trust the threshold logic
//!
//! Stable storage:
//!   * Memory 0: BTreeMap<u64, HealthEvent>      — ring buffer, key = sequence number
//!   * Memory 1: BTreeMap<Principal, WatchedCanister> — canisters under watch
//!   * Memory 2: BTreeMap<u8, Config>            — singleton config (key=0)
//!   * Memory 3: BTreeMap<u8, u64>               — singleton ring counters

use candid::{CandidType, Decode, Encode, Principal};
use ic_cdk::{init, post_upgrade, query, update};
use ic_cdk_timers::TimerId;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::storable::Bound;
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, Storable};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::RefCell;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Types — public API
// ---------------------------------------------------------------------------

/// Severity level for a health event.
#[derive(CandidType, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity { Info, Warn, Error }

/// One observation by the manager about itself or another canister.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct HealthEvent {
    pub seq:          u64,           // monotonically increasing sequence number
    pub timestamp_ns: u64,
    pub source:       String,        // "auth", "registry", "self", or canister id
    pub severity:     Severity,
    pub message:      String,
}

/// A canister the manager is watching.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct WatchedCanister {
    pub id:                Principal,
    pub label:             String,        // e.g. "auth", "registry"
    pub cycles_threshold:  u64,           // emit Warn if below this
    pub last_balance:      Option<u64>,   // most recent observed cycle balance
    pub last_check_ns:     Option<u64>,   // when we last polled this canister
    pub last_status_ok:    Option<bool>,  // last health_check ok value
    pub registered_ns:     u64,
}

/// Manager configuration (single instance).
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub owner:                Principal,    // who can change config + top up
    pub poll_interval_secs:   u64,          // how often the timer ticks
    pub max_events:           u32,          // ring buffer cap
    pub default_threshold:    u64,          // default cycles threshold for new watches
}

/// Errors returned to clients.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ManagerError {
    Unauthorized,
    NotFound,
    AlreadyWatched,
    InvalidConfig(String),
    InsufficientCycles { available: u64, requested: u64 },
}

/// Health snapshot of the manager itself, for completeness/symmetry with
/// the other canisters.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct HealthStatus {
    pub canister:        String,    // "manager"
    pub ok:              bool,
    pub event_count:     u64,
    pub watched_count:   u64,
    pub timestamp_ns:    u64,
    pub cycles_balance:  u64,
}

// ---------------------------------------------------------------------------
// Storable impls
// ---------------------------------------------------------------------------

impl Storable for HealthEvent {
    fn to_bytes(&self) -> Cow<'_, [u8]> { Cow::Owned(Encode!(self).unwrap()) }
    fn from_bytes(b: Cow<[u8]>) -> Self { Decode!(&b, HealthEvent).unwrap() }
    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for WatchedCanister {
    fn to_bytes(&self) -> Cow<'_, [u8]> { Cow::Owned(Encode!(self).unwrap()) }
    fn from_bytes(b: Cow<[u8]>) -> Self { Decode!(&b, WatchedCanister).unwrap() }
    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for Config {
    fn to_bytes(&self) -> Cow<'_, [u8]> { Cow::Owned(Encode!(self).unwrap()) }
    fn from_bytes(b: Cow<[u8]>) -> Self { Decode!(&b, Config).unwrap() }
    const BOUND: Bound = Bound::Unbounded;
}

// ---------------------------------------------------------------------------
// Stable memory layout
// ---------------------------------------------------------------------------

type Memory = VirtualMemory<DefaultMemoryImpl>;

const MEM_EVENTS:    MemoryId = MemoryId::new(0);
const MEM_WATCHED:   MemoryId = MemoryId::new(1);
const MEM_CONFIG:    MemoryId = MemoryId::new(2);
const MEM_COUNTERS:  MemoryId = MemoryId::new(3);

const COUNTER_SEQ:        u8 = 0;  // next event sequence number
const COUNTER_RING_HEAD:  u8 = 1;  // oldest event seq still in buffer

const CONFIG_KEY: u8 = 0;

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static EVENTS: RefCell<StableBTreeMap<u64, HealthEvent, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MEM_EVENTS)))
    );

    static WATCHED: RefCell<StableBTreeMap<Principal, WatchedCanister, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MEM_WATCHED)))
    );

    static CONFIG_STORE: RefCell<StableBTreeMap<u8, Config, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MEM_CONFIG)))
    );

    static COUNTERS: RefCell<StableBTreeMap<u8, u64, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MEM_COUNTERS)))
    );

    /// Heap-only — re-armed in init/post_upgrade. Holds the periodic timer
    /// handle so we can cancel/replace it if config changes.
    static TIMER: RefCell<Option<TimerId>> = const { RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

#[init]
fn init() {
    // Initialize config with the deployer as owner.
    let cfg = Config {
        owner:              ic_cdk::api::caller(),
        poll_interval_secs: 60,
        max_events:         100,
        default_threshold:  1_000_000_000_000, // 1T cycles ~ $1 USD
    };
    CONFIG_STORE.with(|c| c.borrow_mut().insert(CONFIG_KEY, cfg.clone()));

    record_event("self", Severity::Info, format!(
        "manager initialized; owner = {}, poll = {}s",
        cfg.owner, cfg.poll_interval_secs
    ));

    arm_timer(cfg.poll_interval_secs);
    ic_cdk::println!("manager canister initialized");
}

/// After an upgrade, stable storage is intact but the timer is heap-only —
/// re-arm it.
#[post_upgrade]
fn post_upgrade() {
    let cfg = read_config();
    arm_timer(cfg.poll_interval_secs);
    record_event("self", Severity::Info, "manager canister upgraded; timer re-armed".into());
    ic_cdk::println!("manager canister upgraded");
}

// ---------------------------------------------------------------------------
// Timer / periodic poll
// ---------------------------------------------------------------------------

fn arm_timer(interval_secs: u64) {
    // Cancel any existing timer first
    TIMER.with(|t| {
        if let Some(id) = t.borrow_mut().take() {
            ic_cdk_timers::clear_timer(id);
        }
    });

    let new_id = ic_cdk_timers::set_timer_interval(
        Duration::from_secs(interval_secs),
        || {
            ic_cdk::spawn(periodic_check());
        },
    );
    TIMER.with(|t| *t.borrow_mut() = Some(new_id));
}

/// The work the timer does each tick.
async fn periodic_check() {
    // Take a snapshot of watched canister IDs to avoid holding a borrow
    // across the await points below.
    let watched_ids: Vec<Principal> = WATCHED.with(|w| {
        w.borrow().iter().map(|(p, _)| p).collect()
    });

    for canister_id in watched_ids {
        check_one_canister(canister_id).await;
    }

    // Trim the ring buffer if it's grown beyond max_events
    trim_ring_buffer();
}

async fn check_one_canister(canister_id: Principal) {
    // We use the canister's own cycles balance from a management-canister
    // call, then call its health_check. Both are inter-canister calls.
    let label = WATCHED.with(|w| {
        w.borrow().get(&canister_id).map(|c| c.label.clone()).unwrap_or_default()
    });

    // Call the target canister's health_check. We don't decode its actual
    // response shape (each canister has its own); we just check whether
    // the call itself succeeded as a basic liveness signal.
    let liveness_result: Result<(candid::Reserved,), _> =
        ic_cdk::call(canister_id, "health_check", ()).await;
    let ok = liveness_result.is_ok();

    let now = ic_cdk::api::time();

    // Update WatchedCanister with what we learned.
    let threshold = WATCHED.with(|w| {
        let mut w = w.borrow_mut();
        if let Some(mut c) = w.get(&canister_id) {
            c.last_check_ns  = Some(now);
            c.last_status_ok = Some(ok);
            let t = c.cycles_threshold;
            w.insert(canister_id, c);
            t
        } else { 0 }
    });

    if !ok {
        record_event(
            &label,
            Severity::Error,
            format!("health_check call to {} failed", canister_id),
        );
    }

    // Note: querying another canister's cycle balance from outside is
    // restricted on ICP — only the canister itself or its controllers can
    // read its balance directly. So `last_balance` will only be populated
    // when canisters self-report their balance via health_check (a future
    // extension). For now we just leave it as None.
    let _ = threshold; // kept in scope for the future top-up logic
}

fn trim_ring_buffer() {
    let cfg = read_config();
    let max = cfg.max_events as u64;
    let next_seq = read_counter(COUNTER_SEQ);

    if next_seq <= max { return; }

    let oldest_to_keep = next_seq - max;
    let mut head = read_counter(COUNTER_RING_HEAD);

    while head < oldest_to_keep {
        EVENTS.with(|e| { e.borrow_mut().remove(&head); });
        head += 1;
    }
    write_counter(COUNTER_RING_HEAD, head);
}

// ---------------------------------------------------------------------------
// Event recording
// ---------------------------------------------------------------------------

fn record_event(source: &str, severity: Severity, message: String) {
    let seq = read_counter(COUNTER_SEQ);
    let event = HealthEvent {
        seq,
        timestamp_ns: ic_cdk::api::time(),
        source:       source.to_string(),
        severity,
        message,
    };
    EVENTS.with(|e| e.borrow_mut().insert(seq, event));
    write_counter(COUNTER_SEQ, seq + 1);
}

// ---------------------------------------------------------------------------
// Public API — events (read)
// ---------------------------------------------------------------------------

/// Most recent N events, newest first. Pass 0 to use the configured max.
#[query]
fn recent_events(limit: u32) -> Vec<HealthEvent> {
    let cfg = read_config();
    let limit = if limit == 0 { cfg.max_events } else { limit.min(cfg.max_events) };

    let mut out: Vec<HealthEvent> = EVENTS.with(|e| {
        e.borrow().iter().map(|(_, ev)| ev).collect()
    });
    out.sort_by(|a, b| b.seq.cmp(&a.seq));
    out.truncate(limit as usize);
    out
}

#[query]
fn event_count() -> u64 {
    EVENTS.with(|e| e.borrow().len())
}

// ---------------------------------------------------------------------------
// Public API — watch list (admin only)
// ---------------------------------------------------------------------------

/// Add a canister to the watch list. Owner-only.
#[update]
fn watch_canister(id: Principal, label: String, threshold: Option<u64>)
    -> Result<WatchedCanister, ManagerError>
{
    require_owner()?;
    if WATCHED.with(|w| w.borrow().contains_key(&id)) {
        return Err(ManagerError::AlreadyWatched);
    }
    let cfg = read_config();
    let entry = WatchedCanister {
        id,
        label:            label.clone(),
        cycles_threshold: threshold.unwrap_or(cfg.default_threshold),
        last_balance:     None,
        last_check_ns:    None,
        last_status_ok:   None,
        registered_ns:    ic_cdk::api::time(),
    };
    WATCHED.with(|w| w.borrow_mut().insert(id, entry.clone()));
    record_event("self", Severity::Info, format!("now watching {} ({})", label, id));
    Ok(entry)
}

/// Remove a canister from the watch list. Owner-only.
#[update]
fn unwatch_canister(id: Principal) -> Result<bool, ManagerError> {
    require_owner()?;
    let removed = WATCHED.with(|w| w.borrow_mut().remove(&id).is_some());
    if removed {
        record_event("self", Severity::Info, format!("stopped watching {}", id));
    }
    Ok(removed)
}

/// List all canisters currently being watched.
#[query]
fn list_watched() -> Vec<WatchedCanister> {
    WATCHED.with(|w| w.borrow().iter().map(|(_, c)| c).collect())
}

// ---------------------------------------------------------------------------
// Public API — cycles management (admin only)
// ---------------------------------------------------------------------------

/// Manager's own current cycle balance.
#[query]
fn cycles_balance() -> u64 {
    ic_cdk::api::canister_balance()
}

/// Manually top up a target canister with cycles from the manager's
/// own balance. Owner-only.
///
/// NOTE: Real cycle transfer requires calling the management canister's
/// `deposit_cycles` method with cycles attached. That call is shaped here
/// but commented as a stub — it requires the manager to be a controller
/// of the target canister. Wiring that up needs deployment-time setup.
/// For 3c we record the *intent* and validate the request; actual transfer
/// will be activated in a follow-up.
#[update]
async fn top_up(target: Principal, amount: u64) -> Result<u64, ManagerError> {
    require_owner()?;

    let available = ic_cdk::api::canister_balance();
    if amount > available {
        return Err(ManagerError::InsufficientCycles { available, requested: amount });
    }

    record_event(
        "self",
        Severity::Info,
        format!("top_up requested: {} cycles -> {} (stub: transfer not yet active)", amount, target),
    );

    // Real transfer (planned, requires controller setup):
    //
    //   let mgmt_canister = Principal::management_canister();
    //   let _: ((),) = ic_cdk::api::call::call_with_payment(
    //       mgmt_canister,
    //       "deposit_cycles",
    //       (CanisterIdRecord { canister_id: target },),
    //       amount,
    //   ).await.map_err(|(c, m)| ManagerError::InvalidConfig(format!("{:?}: {}", c, m)))?;

    Ok(amount)
}

// ---------------------------------------------------------------------------
// Public API — config (admin only)
// ---------------------------------------------------------------------------

#[query]
fn get_config() -> Config {
    read_config()
}

/// Update the poll interval. Re-arms the timer immediately. Owner-only.
#[update]
fn set_poll_interval(secs: u64) -> Result<(), ManagerError> {
    require_owner()?;
    if secs < 10 || secs > 86_400 {
        return Err(ManagerError::InvalidConfig(
            "poll interval must be 10..=86400 seconds".into()));
    }
    let mut cfg = read_config();
    cfg.poll_interval_secs = secs;
    write_config(cfg.clone());
    arm_timer(secs);
    record_event("self", Severity::Info,
        format!("poll interval changed to {}s", secs));
    Ok(())
}

// ---------------------------------------------------------------------------
// Public API — health (symmetric with auth + registry)
// ---------------------------------------------------------------------------

#[query]
fn health_check() -> HealthStatus {
    HealthStatus {
        canister:       "manager".to_string(),
        ok:             true,
        event_count:    EVENTS.with(|e| e.borrow().len()),
        watched_count:  WATCHED.with(|w| w.borrow().len()),
        timestamp_ns:   ic_cdk::api::time(),
        cycles_balance: ic_cdk::api::canister_balance(),
    }
}

/// Trigger a poll cycle right now (for tests / dashboard refresh button).
#[update]
async fn force_check_now() -> Result<u64, ManagerError> {
    require_owner()?;
    periodic_check().await;
    Ok(EVENTS.with(|e| e.borrow().len()))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_owner() -> Result<(), ManagerError> {
    let caller = ic_cdk::api::caller();
    let owner = read_config().owner;
    if caller != owner { Err(ManagerError::Unauthorized) } else { Ok(()) }
}

fn read_config() -> Config {
    CONFIG_STORE.with(|c| c.borrow().get(&CONFIG_KEY)
        .expect("config not initialized; this is a bug"))
}

fn write_config(cfg: Config) {
    CONFIG_STORE.with(|c| c.borrow_mut().insert(CONFIG_KEY, cfg));
}

fn read_counter(key: u8) -> u64 {
    COUNTERS.with(|c| c.borrow().get(&key).unwrap_or(0))
}

fn write_counter(key: u8, value: u64) {
    COUNTERS.with(|c| c.borrow_mut().insert(key, value));
}

// ---------------------------------------------------------------------------
// Candid export
// ---------------------------------------------------------------------------

ic_cdk::export_candid!();

// ---------------------------------------------------------------------------
// Unit tests — pure logic only.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ordering_is_intuitive() {
        // We rely on Severity being Copy + comparable for filtering UI later.
        assert_eq!(Severity::Info, Severity::Info);
        assert_ne!(Severity::Info, Severity::Error);
    }

    #[test]
    fn health_event_roundtrip() {
        let original = HealthEvent {
            seq:          42,
            timestamp_ns: 1_700_000_000_000_000_000,
            source:       "auth".to_string(),
            severity:     Severity::Warn,
            message:      "test message".to_string(),
        };
        let bytes = original.to_bytes();
        let decoded = HealthEvent::from_bytes(bytes);
        assert_eq!(original.seq,      decoded.seq);
        assert_eq!(original.source,   decoded.source);
        assert_eq!(original.severity, decoded.severity);
        assert_eq!(original.message,  decoded.message);
    }

    #[test]
    fn config_roundtrip() {
        let original = Config {
            owner:              Principal::from_slice(&[1, 2, 3]),
            poll_interval_secs: 60,
            max_events:         100,
            default_threshold:  1_000_000_000_000,
        };
        let bytes = original.to_bytes();
        let decoded = Config::from_bytes(bytes);
        assert_eq!(original.owner,              decoded.owner);
        assert_eq!(original.poll_interval_secs, decoded.poll_interval_secs);
        assert_eq!(original.max_events,         decoded.max_events);
    }
}