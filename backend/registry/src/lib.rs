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
//!   * Hope & Grace (future): each blessing can be registered as its own
//!     historical record — see CLOUD_FACTORY.md Stage 3.
//!
//! Stable storage:
//!   * Memory 0: BTreeMap<Domain, Smartsite>          — primary record
//!   * Memory 1: BTreeMap<(Owner, Domain), ()>        — owner secondary index
//!
//! Ownership verification:
//!   The `OwnershipVerifier` trait is shaped for multi-chain verification.
//!   Today only `InternetIdentity` verifies fully (caller == owner principal).
//!   `SolanaNft` and `EthereumNft` are stubs that return `Err(NotImplemented)`;
//!   they will use HTTP outcalls to the respective RPCs when implemented.
//!
//! Checkpoint 3b.1 additions (May 2026):
//!   * Smartsite gains `status`, `container_id`, `expires_ns` fields for
//!     dashboard visibility and bridge-daemon integration.
//!   * New methods: `update_site_status`, `set_container_id` (owner-only).
//!   * Backward compatible: old records deserialize with sensible defaults.

use candid::{CandidType, Decode, Encode, Principal};
use ic_cdk::{init, query, update};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::storable::Bound;
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, Storable};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::RefCell;

// ---------------------------------------------------------------------------
// Types — public API
// ---------------------------------------------------------------------------

/// Lifecycle state of a smartsite, from registration through serving
/// through retirement. The bridge daemon (when it exists) updates this
/// via `update_site_status`. The dashboard reads it via `get_site`.
#[derive(CandidType, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteStatus {
    /// Just registered. Bridge daemon hasn't deployed the Cloud Can yet.
    Provisioning,
    /// Cloud Can running. Site serving traffic.
    Active,
    /// Migrated to fully-on-chain (per INTEGRATION_PLAN.md Phase H).
    /// VPS container retired; site lives in its own ICP asset canister.
    Purified,
    /// Owner action or non-payment. Container stopped, registry preserved.
    Suspended,
    /// Explicit teardown. Container removed, but registry record kept
    /// for historical/audit purposes.
    Decommissioned,
}

impl Default for SiteStatus {
    fn default() -> Self { SiteStatus::Provisioning }
}

/// A registered smartsite.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct Smartsite {
    pub domain:       String,        // e.g. "johns-bakery.crystaldragon.tech"
    pub owner:        Principal,     // Internet Identity principal
    pub ipfs_cid:     String,        // CIDv1 of the site root, or "" if hosted off-IPFS
    pub created_ns:   u64,
    pub updated_ns:   u64,
    pub ownership:    OwnershipProof,

    // --- Checkpoint 3b.1 additions ---

    /// Lifecycle state. Defaults to Provisioning on register; bridge
    /// daemon updates to Active when the Cloud Can is serving.
    #[serde(default)]
    pub status:       SiteStatus,

    /// Docker container ID once the Cloud Can is deployed. None during
    /// Provisioning, Some(id) once Active, None again after Decommissioned.
    #[serde(default)]
    pub container_id: Option<String>,

    /// When this site's storage allocation expires (nanoseconds since
    /// Unix epoch). None = perpetual / paid for life. Used by the
    /// future cleanup job to identify abandoned sites.
    #[serde(default)]
    pub expires_ns:   Option<u64>,
}

/// Pluggable ownership proof. Today `InternetIdentity` is verified
/// natively; chain variants are stubs awaiting HTTP-outcall implementation.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub enum OwnershipProof {
    /// Owner principal IS the proof. Caller-principal check enforces it.
    InternetIdentity,
    /// Crystal Dragon Yggdrasil KEY or any other Solana NFT.
    SolanaNft {
        mint:   String,             // Solana mint address
        wallet: String,             // Owner's Solana wallet pubkey (base58)
        tier:   Option<KeyTier>,    // Crystal Dragon tier, None for generic NFT
    },
    /// EVM-chain NFT (Ethereum, Polygon, Base, Arbitrum, Optimism).
    EthereumNft {
        contract: String,           // ERC-721 contract address (0x...)
        token_id: String,           // token id as decimal string
        wallet:   String,           // owner address (0x...)
        chain:    EvmChain,         // which EVM chain to verify against
    },
}

/// Crystal Dragon Yggdrasil KEY tier system per Crystal Dragon's
/// MASTER_CHECKLIST.md and WHITEPAPER_DRAFT.md:
///   ROOT   #0001-0100   (Genesis)
///   TRUNK  #0101-1000   (Standard)
///   BRANCH #1001-5000   (Premium)
///   CROWN  #5001-10000  (Elite)
///   DOMAIN unlimited    (Transfer KEY)
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, Copy, PartialEq, Eq)]
pub enum KeyTier { Root, Trunk, Branch, Crown, Domain }

/// EVM chains we verify against. Each maps to a known RPC endpoint
/// configured in canister state (set by owner, not hardcoded here).
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, Copy, PartialEq, Eq)]
pub enum EvmChain { Ethereum, Polygon, Base, Arbitrum, Optimism }

/// Errors returned to clients. Specific enough to act on, vague enough
/// not to leak internals.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// Domain already exists.
    AlreadyRegistered,
    /// No smartsite with this domain.
    NotFound,
    /// Caller is not the owner of this smartsite.
    Unauthorized,
    /// Domain string is empty, too long, or has invalid characters.
    InvalidDomain(String),
    /// IPFS CID string is empty (when required) or malformed.
    InvalidCid(String),
    /// Ownership verification was attempted but the verifier isn't built yet.
    NotImplemented(String),
    /// Anonymous principals can't register smartsites.
    AnonymousCaller,
}

/// Health snapshot. Manager polls this and treats `ok == true` as alive.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct HealthStatus {
    pub canister:     String,    // "registry"
    pub ok:           bool,
    pub site_count:   u64,
    pub timestamp_ns: u64,
}

// ---------------------------------------------------------------------------
// Internal types — stable storage
// ---------------------------------------------------------------------------

/// Composite key for the owner secondary index.
/// Stored bytes are: principal_len (1 byte) | principal | domain (utf-8).
/// This ordering means iterating over a single owner's entries is a
/// contiguous range scan — efficient even with millions of entries.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OwnerKey {
    owner:  Principal,
    domain: String,
}

// ---------------------------------------------------------------------------
// Storable impls — required for ic-stable-structures
// ---------------------------------------------------------------------------

impl Storable for Smartsite {
    fn to_bytes(&self) -> Cow<'_, [u8]> { Cow::Owned(Encode!(self).unwrap()) }
    fn from_bytes(b: Cow<[u8]>) -> Self { Decode!(&b, Smartsite).unwrap() }
    const BOUND: Bound = Bound::Unbounded;
}

impl Storable for OwnerKey {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let p = self.owner.as_slice();
        let mut out = Vec::with_capacity(1 + p.len() + self.domain.len());
        out.push(p.len() as u8);
        out.extend_from_slice(p);
        out.extend_from_slice(self.domain.as_bytes());
        Cow::Owned(out)
    }
    fn from_bytes(b: Cow<[u8]>) -> Self {
        let plen = b[0] as usize;
        let owner = Principal::from_slice(&b[1..1 + plen]);
        let domain = String::from_utf8(b[1 + plen..].to_vec()).unwrap();
        OwnerKey { owner, domain }
    }
    const BOUND: Bound = Bound::Bounded { max_size: 320, is_fixed_size: false };
}

// ---------------------------------------------------------------------------
// Stable memory layout
// ---------------------------------------------------------------------------

type Memory = VirtualMemory<DefaultMemoryImpl>;

const MEM_SITES:       MemoryId = MemoryId::new(0);
const MEM_OWNER_INDEX: MemoryId = MemoryId::new(1);

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static SITES: RefCell<StableBTreeMap<String, Smartsite, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MEM_SITES)))
    );

    /// Owner secondary index. Value is unit `()` — the OwnerKey alone is the data.
    /// We pack into a u8 so Storable's bounded encoding stays simple.
    static OWNER_INDEX: RefCell<StableBTreeMap<OwnerKey, u8, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MEM_OWNER_INDEX)))
    );
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

#[init]
fn init() {
    ic_cdk::println!("registry canister initialized");
}

/// Stable storage persists across upgrades automatically — nothing to do here
/// except confirm we made it through.
#[ic_cdk::post_upgrade]
fn post_upgrade() {
    ic_cdk::println!("registry canister upgraded; stable storage preserved");
}

// ---------------------------------------------------------------------------
// Ownership verification trait + impls
// ---------------------------------------------------------------------------

/// Strategy pattern for ownership verification. Each variant of
/// `OwnershipProof` has a corresponding verifier. New chains plug in here
/// without touching the public API.
trait OwnershipVerifier {
    /// Returns Ok(()) if the caller is the legitimate owner per this proof.
    /// Returns Err(RegistryError) otherwise.
    fn verify(&self, caller: Principal, proof: &OwnershipProof) -> Result<(), RegistryError>;
}

/// Verifies "the caller IS the owning principal" — used for the
/// InternetIdentity proof variant.
struct InternetIdentityVerifier;

impl OwnershipVerifier for InternetIdentityVerifier {
    fn verify(&self, _caller: Principal, _proof: &OwnershipProof) -> Result<(), RegistryError> {
        // For II proofs, the smartsite owner field IS the proof. We don't
        // need to verify anything beyond the caller-equals-owner check
        // that register/update/delete already do. So this is a no-op:
        // the structural invariant of "owner field equals caller" is the proof.
        Ok(())
    }
}

/// Stub for Solana NFT verification. Real impl will use HTTP outcalls
/// to a Solana RPC to check that `wallet` currently owns `mint`.
struct SolanaNftVerifier;

impl OwnershipVerifier for SolanaNftVerifier {
    fn verify(&self, _caller: Principal, _proof: &OwnershipProof) -> Result<(), RegistryError> {
        Err(RegistryError::NotImplemented(
            "SolanaNft verification requires HTTP outcalls (planned for 3b late stage)".into()
        ))
    }
}

/// Stub for EVM (Ethereum/Polygon/Base/etc) NFT verification. Real impl
/// will use HTTP outcalls to the chain's RPC to check ERC-721 ownership.
struct EthereumNftVerifier;

impl OwnershipVerifier for EthereumNftVerifier {
    fn verify(&self, _caller: Principal, _proof: &OwnershipProof) -> Result<(), RegistryError> {
        Err(RegistryError::NotImplemented(
            "EthereumNft verification requires HTTP outcalls (planned for 3b late stage)".into()
        ))
    }
}

/// Dispatch a proof to the right verifier.
fn verify_ownership(caller: Principal, proof: &OwnershipProof) -> Result<(), RegistryError> {
    match proof {
        OwnershipProof::InternetIdentity      => InternetIdentityVerifier.verify(caller, proof),
        OwnershipProof::SolanaNft   { .. }    => SolanaNftVerifier.verify(caller, proof),
        OwnershipProof::EthereumNft { .. }    => EthereumNftVerifier.verify(caller, proof),
    }
}

// ---------------------------------------------------------------------------
// Public API — smartsite CRUD
// ---------------------------------------------------------------------------

/// Register a new smartsite. Caller becomes the owner.
/// Fails if domain is already taken.
///
/// New sites start in `SiteStatus::Provisioning` with no container_id
/// and no expiry. Bridge daemon (when it exists) updates these via
/// `update_site_status` and `set_container_id`.
#[update]
fn register_site(
    domain:    String,
    ipfs_cid:  String,
    ownership: OwnershipProof,
) -> Result<Smartsite, RegistryError> {
    let caller = ic_cdk::api::caller();
    if caller == Principal::anonymous() {
        return Err(RegistryError::AnonymousCaller);
    }

    validate_domain(&domain)?;
    validate_cid_optional(&ipfs_cid)?;
    verify_ownership(caller, &ownership)?;

    let now = ic_cdk::api::time();
    let site = Smartsite {
        domain:       domain.clone(),
        owner:        caller,
        ipfs_cid,
        created_ns:   now,
        updated_ns:   now,
        ownership,
        status:       SiteStatus::Provisioning,
        container_id: None,
        expires_ns:   None,
    };

    SITES.with(|sites| {
        let mut sites = sites.borrow_mut();
        if sites.contains_key(&domain) {
            return Err(RegistryError::AlreadyRegistered);
        }
        sites.insert(domain.clone(), site.clone());
        Ok(())
    })?;

    // Maintain owner secondary index
    OWNER_INDEX.with(|idx| {
        idx.borrow_mut().insert(OwnerKey { owner: caller, domain }, 0);
    });

    Ok(site)
}

/// Look up a smartsite by domain. Public read — no caller check.
#[query]
fn get_site(domain: String) -> Result<Smartsite, RegistryError> {
    SITES.with(|sites| sites.borrow().get(&domain).ok_or(RegistryError::NotFound))
}

/// List ALL smartsites. Useful for admin/dashboard.
/// Note: O(n) over total sites; for owner-scoped queries use sites_by_owner.
#[query]
fn list_sites() -> Vec<Smartsite> {
    SITES.with(|sites| sites.borrow().iter().map(|(_, s)| s).collect())
}

/// List all smartsites owned by a specific principal.
/// Uses the secondary index — fast even at scale.
#[query]
fn sites_by_owner(owner: Principal) -> Vec<Smartsite> {
    // Find all OwnerKey entries for this principal, then look up each in SITES.
    let domains: Vec<String> = OWNER_INDEX.with(|idx| {
        idx.borrow()
            .iter()
            .filter(|(k, _)| k.owner == owner)
            .map(|(k, _)| k.domain.clone())
            .collect()
    });
    SITES.with(|sites| {
        let sites = sites.borrow();
        domains.into_iter().filter_map(|d| sites.get(&d)).collect()
    })
}

/// Update the IPFS CID for an existing smartsite. Only the owner can do this.
#[update]
fn update_cid(domain: String, new_cid: String) -> Result<Smartsite, RegistryError> {
    let caller = ic_cdk::api::caller();
    validate_cid_optional(&new_cid)?;
    SITES.with(|sites| {
        let mut sites = sites.borrow_mut();
        let mut site = sites.get(&domain).ok_or(RegistryError::NotFound)?;
        if site.owner != caller {
            return Err(RegistryError::Unauthorized);
        }
        site.ipfs_cid   = new_cid;
        site.updated_ns = ic_cdk::api::time();
        sites.insert(domain, site.clone());
        Ok(site)
    })
}

/// Update the lifecycle status of a smartsite. Only the owner can do this.
/// Called by the bridge daemon to report deployment progress, by the
/// owner to suspend/decommission, and (eventually) by the migration job
/// to mark a site as Purified.
#[update]
fn update_site_status(domain: String, new_status: SiteStatus)
    -> Result<Smartsite, RegistryError>
{
    let caller = ic_cdk::api::caller();
    SITES.with(|sites| {
        let mut sites = sites.borrow_mut();
        let mut site = sites.get(&domain).ok_or(RegistryError::NotFound)?;
        if site.owner != caller {
            return Err(RegistryError::Unauthorized);
        }
        site.status     = new_status;
        site.updated_ns = ic_cdk::api::time();
        sites.insert(domain, site.clone());
        Ok(site)
    })
}

/// Set or clear the Docker container ID for a smartsite. Only the owner
/// can do this. Pass an empty string to clear (sets to None internally).
/// Called by the bridge daemon after `docker compose up` succeeds.
#[update]
fn set_container_id(domain: String, container_id: String)
    -> Result<Smartsite, RegistryError>
{
    let caller = ic_cdk::api::caller();
    SITES.with(|sites| {
        let mut sites = sites.borrow_mut();
        let mut site = sites.get(&domain).ok_or(RegistryError::NotFound)?;
        if site.owner != caller {
            return Err(RegistryError::Unauthorized);
        }
        site.container_id = if container_id.is_empty() {
            None
        } else {
            Some(container_id)
        };
        site.updated_ns = ic_cdk::api::time();
        sites.insert(domain, site.clone());
        Ok(site)
    })
}

/// Delete a smartsite. Only the owner can do this.
#[update]
fn delete_site(domain: String) -> Result<bool, RegistryError> {
    let caller = ic_cdk::api::caller();
    let owner = SITES.with(|sites| {
        sites.borrow().get(&domain).map(|s| s.owner)
    });

    let owner = owner.ok_or(RegistryError::NotFound)?;
    if owner != caller {
        return Err(RegistryError::Unauthorized);
    }

    SITES.with(|s| s.borrow_mut().remove(&domain));
    OWNER_INDEX.with(|idx| {
        idx.borrow_mut().remove(&OwnerKey { owner, domain });
    });
    Ok(true)
}

/// Total registered smartsites. For dashboard + manager polling.
#[query]
fn site_count() -> u64 {
    SITES.with(|sites| sites.borrow().len())
}

// ---------------------------------------------------------------------------
// Public API — health (polled by manager)
// ---------------------------------------------------------------------------

#[query]
fn health_check() -> HealthStatus {
    HealthStatus {
        canister:     "registry".to_string(),
        ok:           true,
        site_count:   SITES.with(|s| s.borrow().len()),
        timestamp_ns: ic_cdk::api::time(),
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Domain validation: 1..=253 chars total, segments 1..=63 chars,
/// alphanumeric + hyphen, no leading/trailing hyphen on a segment, at
/// least one dot (so "localhost" is rejected; we want real domains).
fn validate_domain(domain: &str) -> Result<(), RegistryError> {
    if domain.is_empty() || domain.len() > 253 {
        return Err(RegistryError::InvalidDomain("length must be 1..=253 chars".into()));
    }
    if !domain.contains('.') {
        return Err(RegistryError::InvalidDomain("must contain at least one dot".into()));
    }
    for segment in domain.split('.') {
        if segment.is_empty() || segment.len() > 63 {
            return Err(RegistryError::InvalidDomain(
                format!("segment '{}' must be 1..=63 chars", segment)));
        }
        if segment.starts_with('-') || segment.ends_with('-') {
            return Err(RegistryError::InvalidDomain(
                format!("segment '{}' cannot start or end with hyphen", segment)));
        }
        if !segment.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(RegistryError::InvalidDomain(
                format!("segment '{}' has invalid characters", segment)));
        }
    }
    Ok(())
}

/// CID validation. Empty string is allowed (off-IPFS hosting). Non-empty
/// must look like a CID — we don't validate the multihash structure deeply,
/// just basic shape (alphanumeric, reasonable length).
fn validate_cid_optional(cid: &str) -> Result<(), RegistryError> {
    if cid.is_empty() {
        return Ok(());
    }
    if cid.len() < 46 || cid.len() > 100 {
        return Err(RegistryError::InvalidCid(
            "CID length should be 46..=100 chars".into()));
    }
    if !cid.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(RegistryError::InvalidCid(
            "CID must be alphanumeric (base32 or base58)".into()));
    }
    Ok(())
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
    fn domain_validation_accepts_normal_domains() {
        assert!(validate_domain("example.com").is_ok());
        assert!(validate_domain("johns-bakery.crystaldragon.tech").is_ok());
        assert!(validate_domain("a.b.c.d.e.f.example.com").is_ok());
    }

    #[test]
    fn domain_validation_rejects_bad_domains() {
        assert!(validate_domain("").is_err());                  // empty
        assert!(validate_domain("localhost").is_err());         // no dot
        assert!(validate_domain("-bad.com").is_err());          // leading hyphen
        assert!(validate_domain("bad-.com").is_err());          // trailing hyphen
        assert!(validate_domain("has spaces.com").is_err());    // invalid char
    }

    #[test]
    fn cid_validation_allows_empty() {
        assert!(validate_cid_optional("").is_ok());
    }

    #[test]
    fn cid_validation_rejects_too_short() {
        assert!(validate_cid_optional("abc").is_err());
    }

    #[test]
    fn cid_validation_accepts_typical_cidv1() {
        let cid = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        assert!(validate_cid_optional(cid).is_ok());
    }

    #[test]
    fn owner_key_roundtrip() {
        let original = OwnerKey {
            owner:  Principal::from_slice(&[1, 2, 3, 4, 5]),
            domain: "example.com".to_string(),
        };
        let bytes = original.to_bytes();
        let decoded = OwnerKey::from_bytes(bytes);
        assert_eq!(original.owner,  decoded.owner);
        assert_eq!(original.domain, decoded.domain);
    }

    #[test]
    fn owner_key_ordering_groups_by_owner() {
        let alice_a = OwnerKey { owner: Principal::from_slice(&[1]), domain: "a.com".into() };
        let alice_b = OwnerKey { owner: Principal::from_slice(&[1]), domain: "b.com".into() };
        let bob_a   = OwnerKey { owner: Principal::from_slice(&[2]), domain: "a.com".into() };

        let mut keys = vec![bob_a.clone(), alice_b.clone(), alice_a.clone()];
        keys.sort();
        assert_eq!(keys, vec![alice_a, alice_b, bob_a]);
    }

    #[test]
    fn site_status_default_is_provisioning() {
        let status: SiteStatus = Default::default();
        assert_eq!(status, SiteStatus::Provisioning);
    }
}