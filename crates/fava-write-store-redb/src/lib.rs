//! Durable Redb authority for accepted publication obligations and receipts.

use std::collections::BTreeMap;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::Path;
use std::sync::{Arc, Mutex};

use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError, SourceChangeFuture,
    SourceChanges, SourceEvent, SourceKind, SourceRevision, SourceSnapshot, SourceStatus,
};
use fava_state::EventCoordinate;
use fava_write::{
    EventId, PublicKey, Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome,
    EventEdit, Timestamp, UnsignedEvent,
};
use fava_write_store::WriteStoreError;
use redb::Database;
use tokio::sync::{broadcast, watch};

mod lifecycle;
mod ops;
mod recovery;
mod schema;
mod semantic;
mod semantic_acceptance;
mod semantic_composition;
mod signing;
mod validation;

const RECEIPT_CHANGE_CAPACITY: usize = 256;

type SemanticCustody = (
    Vec<EventEdit>,
    PublicKey,
    Option<(EventId, Timestamp)>,
    Option<EventId>,
    Option<(
        Option<EventEdit>,
        UnsignedEvent,
        Option<(EventId, Timestamp)>,
        Option<fava_routing::RoutePlan>,
    )>,
);

/// Redb write store with bounded active work and retained terminal receipts.
pub struct RedbWriteStore {
    database: Arc<Database>,
    state: Mutex<StoreState>,
    limits: StoreLimits,
    latest: watch::Sender<Arc<SourceSnapshot>>,
    receipt_changes: broadcast::Sender<(ReceiptId, Option<Receipt>)>,
}

/// The whole in-memory ledger: coordinate reservations, retained receipts, which
/// receipt owns which coordinate, and in-flight replaceable-event custody.
#[derive(Clone, Debug)]
struct StoreState {
    revision: u64,
    next_identity: NonZeroU64,
    next_reservation: u64,
    reservations: BTreeMap<u64, EventCoordinate>,
    receipts: BTreeMap<ReceiptId, Receipt>,
    coordinates: BTreeMap<EventCoordinate, ReceiptId>,
    semantics: BTreeMap<ReceiptId, SemanticCustody>,
}

#[derive(Clone, Copy, Debug)]
struct StoreLimits {
    active: NonZeroUsize,
    terminal: NonZeroUsize,
}

impl RedbWriteStore {
    /// Open or create the standard durable profile at one exact path.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when durable state cannot open or recover.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WriteStoreError> {
        let standard_limit = NonZeroUsize::new(10_000).ok_or_else(|| {
            WriteStoreError::Refused("standard bound must be non-zero".to_owned())
        })?;
        Self::open_bounded(path, standard_limit, standard_limit)
    }

    /// Open with exact active and terminal-receipt bounds.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when durable state cannot open or recover.
    pub fn open_bounded(
        path: impl AsRef<Path>,
        active: NonZeroUsize,
        terminal: NonZeroUsize,
    ) -> Result<Self, WriteStoreError> {
        let path = path.as_ref();
        let is_new = !path.exists();
        let database = Arc::new(Database::create(path).map_err(refused)?);
        schema::initialize(&database, is_new)?;
        let (next_identity, receipts, coordinates, semantics) = schema::load(&database)?;
        let mut state = StoreState {
            revision: 0,
            next_identity,
            next_reservation: 1,
            reservations: BTreeMap::new(),
            receipts,
            coordinates,
            semantics,
        };
        let recovered = recover_ambiguous_delivery(&mut state);
        lifecycle::validate_recovered_bounds(&state, active.get(), terminal.get())?;
        if !recovered.is_empty() {
            schema::persist_existing(&database, &state, &recovered)?;
        }
        let (latest, _) = watch::channel(Arc::new(snapshot(&state)));
        let (receipt_changes, _) = broadcast::channel(RECEIPT_CHANGE_CAPACITY);
        let store = Self {
            database,
            state: Mutex::new(state),
            limits: StoreLimits { active, terminal },
            latest,
            receipt_changes,
        };
        store.recover_authorized_signing()?;
        Ok(store)
    }

    fn commit_accept(
        &self,
        next_identity: NonZeroU64,
        receipt: &Receipt,
        semantic: Option<&SemanticCustody>,
        removals: &[ReceiptId],
    ) -> Result<(), WriteStoreError> {
        schema::commit_accept(&self.database, next_identity, receipt, semantic, removals)
    }

    fn commit_update(
        &self,
        receipt: Option<&Receipt>,
        semantic: Option<&SemanticCustody>,
        removals: &[ReceiptId],
    ) -> Result<(), WriteStoreError> {
        schema::commit_update(&self.database, receipt, semantic, removals)
    }

    fn publish_snapshot(&self, state: &StoreState) {
        self.latest.send_replace(Arc::new(snapshot(state)));
    }

    fn publish_receipt(&self, receipt: Option<Receipt>, receipt_id: ReceiptId) {
        let _ = self.receipt_changes.send((receipt_id, receipt));
    }
}

impl QuerySource for RedbWriteStore {
    fn open(&self, _query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        let receiver = self.latest.subscribe();
        let initial = receiver.borrow().as_ref().clone();
        Ok(OpenedQuerySource {
            initial,
            changes: Box::new(WatchChanges {
                receiver,
                closed: false,
            }),
        })
    }
}

struct WatchChanges {
    receiver: watch::Receiver<Arc<SourceSnapshot>>,
    closed: bool,
}

impl SourceChanges for WatchChanges {
    fn next_change(&mut self) -> SourceChangeFuture<'_> {
        Box::pin(async move {
            if self.closed {
                return Err(QuerySourceClosed::local_close());
            }
            if self.receiver.changed().await.is_err() {
                self.closed = true;
                return Err(QuerySourceClosed::provider_closed());
            }
            Ok(self.receiver.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {
        self.closed = true;
    }
}

fn recover_ambiguous_delivery(state: &mut StoreState) -> Vec<ReceiptId> {
    let mut recovered = Vec::new();
    let mut released = Vec::new();
    for receipt in state.receipts.values_mut() {
        let mut changed = false;
        for outcome in receipt.current.publication.destinations.values_mut() {
            if matches!(outcome, RelayDeliveryOutcome::Attempting) {
                *outcome = RelayDeliveryOutcome::Unknown {
                    reason: "process ended after attempt authorization before outcome commit"
                        .to_owned(),
                };
                changed = true;
            }
        }
        if changed {
            lifecycle::settle(receipt);
            if receipt.is_terminal() {
                released.push(receipt.receipt_id);
            }
            recovered.push(receipt.receipt_id);
        }
    }
    for receipt_id in released {
        release_semantic_coordinate(state, receipt_id);
    }
    recovered
}

fn release_semantic(state: &mut StoreState, receipt_id: ReceiptId) {
    if let Some((edits, author, _, _, _)) = state.semantics.remove(&receipt_id)
        && let Some(edit) = edits.last()
    {
        state
            .coordinates
            .remove(&semantic::edit_coordinate(edit, author));
    }
}

fn release_semantic_coordinate(state: &mut StoreState, receipt_id: ReceiptId) {
    if let Some((edits, author, _, _, _)) = state.semantics.get(&receipt_id)
        && let Some(edit) = edits.last()
    {
        state
            .coordinates
            .remove(&semantic::edit_coordinate(edit, *author));
    }
}

fn snapshot(state: &StoreState) -> SourceSnapshot {
    SourceSnapshot {
        kind: SourceKind::WriteStore,
        revision: SourceRevision(state.revision),
        status: SourceStatus::Open,
        events: state
            .receipts
            .values()
            .filter(|receipt| !matches!(receipt.outcome, ReceiptOutcome::Cancelled))
            .map(|receipt| SourceEvent::Local(receipt.current.clone()))
            .collect(),
        retractions: Vec::new(),
    }
}

fn refused(error: impl std::fmt::Display) -> WriteStoreError {
    WriteStoreError::Refused(error.to_string())
}
