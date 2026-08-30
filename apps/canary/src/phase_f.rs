//! Phase F — Cross-implementation relay testing.
//!
//! Runs the NIP-29 group lifecycle against relay29 and communities-relay,
//! proves NIP-42 AUTH handling, and verifies crash-recovery (SIGKILL-fava)
//! against relay29.
//!
//! # Gate summary
//!
//! | Gate | Relay | Status produced |
//! |------|-------|-----------------|
//! | relay29 NIP-29 lifecycle | relay29 (~/src/relay29) | documented gap — relay uses pre-spec custom kinds |
//! | communities-relay NIP-29 lifecycle | communities-relay | wire transcript + lifecycle complete |
//! | NIP-42 AUTH proof | communities-relay | AUTH handshake in transcript |
//! | Crash-recovery | relay29 | no-dup EVENT on wire after recovery |

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, RelayUrl};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher_nip01::{Nip01Publisher, Nip42Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_session::Session;
use fava_signer::Signer;
use fava_signer_local::LocalSigner;
use fava_simple_groups::{
    MetadataEdit, SimpleGroup, create_group, delete_group, edit_metadata, invite, join_request,
    leave_group, put_user, remove_user,
};
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write::ReceiptOutcome;
use fava_write_store_redb::RedbWriteStore;
use nostr::key::Keys;
use serde_json::json;

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::go_relay::{GoRelayProcess, GoRelaySupervisor};
use crate::publication_support::{
    spawn_crash_child, wait_child_marker, wait_recovered_terminal, wait_terminal,
};
use crate::{CanaryError, CanaryResult, WireProxy, deterministic_keys, reserve_port, wire};

/// Options for Phase F scenarios.
#[derive(Clone, Debug)]
pub struct PhaseFOptions {
    /// Path to the relay29 binary.
    pub relay29_binary: PathBuf,
    /// Path to the communities-relay binary.
    pub communities_relay_binary: PathBuf,
    /// Caller-selected seed.
    pub seed: String,
    /// Parent directory for evidence bundles.
    pub runs_directory: PathBuf,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Deterministic relay private key (used in all Phase F runs for reproducibility).
/// 32-byte big-endian, 64 lowercase hex chars.
const RELAY_PRIVKEY: &str = "f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6";

include!("phase_f/gate1_relay29_lifecycle.rs");

include!("phase_f/gate2_3_communities_lifecycle.rs");

include!("phase_f/gate4_crash_recovery.rs");

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a Fava with `Nip42Publisher` (handles AUTH challenge → kind-22242 response).
fn assembly_nip42(database: PathBuf, keys: &Keys) -> CanaryResult<Fava> {
    let signer: Arc<dyn Signer> = Arc::new(LocalSigner::new(keys.clone()));
    let session =
        Session::new([Arc::clone(&signer)]).map_err(|e| CanaryError::new(e.to_string()))?;
    Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(
            RedbWriteStore::open(database).map_err(|e| CanaryError::new(e.to_string()))?,
        ))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
        .publisher(Arc::new(Nip42Publisher::new(session)))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .signers([signer])
        .build()
        .map_err(|e| CanaryError::new(e.to_string()))
}

/// Summarise a receipt as acknowledged/rejected/ambiguous for JSON.
fn receipt_summary(receipt: &fava::Receipt) -> ReceiptOutcome {
    receipt.outcome
}

/// Extract the first relay rejection message from a receipt, if any.
fn reject_message(receipt: &fava::Receipt) -> String {
    use fava::RelayDeliveryOutcome;
    receipt
        .destinations()
        .values()
        .find_map(|outcome| {
            if let RelayDeliveryOutcome::Rejected { message, .. } = outcome {
                Some(message.clone())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Count wire-log lines containing `fragment` (simple substring search).
fn count_wire_frames(log: &std::path::Path, fragment: &str) -> usize {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains(fragment))
        .count()
}

/// Count lines where direction is relay→client and payload contains `fragment`.
fn count_relay_to_client_frames(log: &std::path::Path, fragment: &str) -> usize {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains("relay_to_client") && line.contains(fragment))
        .count()
}

/// Count lines where direction is client→relay and payload contains `fragment`.
fn count_client_to_relay_frames(log: &std::path::Path, fragment: &str) -> usize {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains("client_to_relay") && line.contains(fragment))
        .count()
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
