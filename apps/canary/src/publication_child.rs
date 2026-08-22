//! Crash-child entry point and controllable exact signer for M5 canaries.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use fava::{EventBuilder, Fava};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_signer_local::LocalSigner;
use fava_state::RelayUrl;
use fava_transport_websocket::WebSocketTransport;
use fava_write::{Event, Kind, PublicKey, Timestamp, UnsignedEvent};
use fava_write_store_redb::RedbWriteStore;
use nostr::key::Keys;
use serde::Serialize;
use tokio::sync::watch;

use crate::{CanaryError, CanaryResult, deterministic_keys};

/// Run the hidden child stopped by the crash-after-acceptance supervisor.
///
/// # Errors
///
/// Returns an exact argument, provider, signing, persistence, or marker failure.
pub async fn run_crash_child(arguments: Vec<String>) -> CanaryResult<()> {
    let [database, marker, relay, seed] = arguments.as_slice() else {
        return Err(CanaryError::new(
            "crash-child requires DATABASE MARKER RELAY SEED",
        ));
    };
    let keys = deterministic_keys(seed)?;
    let relay = RelayUrl::parse(relay).map_err(error)?;
    let store = Arc::new(RedbWriteStore::open(PathBuf::from(database)).map_err(error)?);
    let signer = Arc::new(GatedSigner::new(keys.clone()));
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(WebSocketTransport::default()))
        .signer(signer)
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .map_err(error)?;
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .created_at(Timestamp::now())
        .content(format!("Fava M5 crash recovery {seed}"))
        .build()
        .map_err(error)?;
    let event_id = event
        .id
        .ok_or_else(|| CanaryError::new("checked builder produced no event id"))?;
    let accepted = fava
        .to([relay])
        .map_err(error)?
        .publish(event)
        .map_err(error)?;
    let marker = PathBuf::from(marker);
    let marker_temporary = marker.with_extension("json.tmp");
    std::fs::write(
        &marker_temporary,
        serde_json::to_vec(&AcceptedMarker {
            receipt_id: accepted.receipt_id().as_u64(),
            event_id: event_id.to_hex(),
        })?,
    )?;
    std::fs::rename(marker_temporary, marker)?;
    std::future::pending::<()>().await;
    Ok(())
}

#[derive(Serialize)]
struct AcceptedMarker {
    receipt_id: u64,
    event_id: String,
}

pub(crate) struct GatedSigner {
    inner: LocalSigner,
    gate: watch::Sender<bool>,
    calls: AtomicU64,
}

impl GatedSigner {
    pub(crate) fn new(keys: Keys) -> Self {
        let (gate, _) = watch::channel(false);
        Self {
            inner: LocalSigner::new(keys),
            gate,
            calls: AtomicU64::new(0),
        }
    }

    pub(crate) fn new_released(keys: Keys) -> Self {
        let signer = Self::new(keys);
        signer.release();
        signer
    }

    pub(crate) fn release(&self) {
        self.gate.send_replace(true);
    }

    pub(crate) fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Signer for GatedSigner {
    fn public_key(&self) -> PublicKey {
        self.inner.public_key()
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        &self,
        event: UnsignedEvent,
        mut cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut gate = self.gate.subscribe();
        Box::pin(async move {
            if !*gate.borrow() {
                tokio::select! {
                    biased;
                    _ = cancel.changed() => return Err(SignerError::Cancelled),
                    _ = gate.changed() => {}
                }
            }
            self.inner.sign_event(event, cancel).await
        })
    }
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
