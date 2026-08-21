//! Durable Redb authority for accepted publication obligations and receipts.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::{Arc, Mutex};

use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError, SourceChangeFuture,
    SourceChanges, SourceEvent, SourceKind, SourceRevision, SourceSnapshot, SourceStatus,
};
use fava_write::{Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome};
use fava_write_store::WriteStoreError;
use redb::{Database, Durability, ReadableDatabase, ReadableTable, TableDefinition};
use tokio::sync::{broadcast, watch};

mod ops;

const RECEIPTS: TableDefinition<u64, &[u8]> = TableDefinition::new("receipts");
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
const NEXT_ID: &str = "next_id";
const RECEIPT_CHANGE_CAPACITY: usize = 256;

/// Redb write store with bounded active work and retained terminal receipts.
pub struct RedbWriteStore {
    database: Arc<Database>,
    state: Mutex<StoreState>,
    limits: StoreLimits,
    latest: watch::Sender<Arc<SourceSnapshot>>,
    receipt_changes: broadcast::Sender<(ReceiptId, Option<Receipt>)>,
}

#[derive(Clone, Debug)]
struct StoreState {
    revision: u64,
    next_identity: u64,
    receipts: BTreeMap<ReceiptId, Receipt>,
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
        let database = Arc::new(Database::create(path).map_err(refused)?);
        initialize(&database)?;
        let (next_identity, mut receipts) = load(&database)?;
        let recovered = recover_ambiguous(&mut receipts);
        if !recovered.is_empty() {
            persist_many(&database, recovered.iter())?;
        }
        let state = StoreState {
            revision: 0,
            next_identity,
            receipts,
        };
        let (latest, _) = watch::channel(Arc::new(snapshot(&state)));
        let (receipt_changes, _) = broadcast::channel(RECEIPT_CHANGE_CAPACITY);
        Ok(Self {
            database,
            state: Mutex::new(state),
            limits: StoreLimits { active, terminal },
            latest,
            receipt_changes,
        })
    }

    fn commit_accept(&self, next_identity: u64, receipt: &Receipt) -> Result<(), WriteStoreError> {
        let mut transaction = self.database.begin_write().map_err(refused)?;
        transaction
            .set_durability(Durability::Immediate)
            .map_err(refused)?;
        {
            let mut receipts = transaction.open_table(RECEIPTS).map_err(refused)?;
            let bytes = serde_json::to_vec(receipt).map_err(refused)?;
            receipts
                .insert(receipt.receipt_id.as_u64(), bytes.as_slice())
                .map_err(refused)?;
        }
        {
            let mut meta = transaction.open_table(META).map_err(refused)?;
            meta.insert(NEXT_ID, next_identity).map_err(refused)?;
        }
        transaction.commit().map_err(refused)
    }

    fn commit_update(
        &self,
        receipt: Option<&Receipt>,
        removals: &[ReceiptId],
    ) -> Result<(), WriteStoreError> {
        let mut transaction = self.database.begin_write().map_err(refused)?;
        transaction
            .set_durability(Durability::Immediate)
            .map_err(refused)?;
        {
            let mut table = transaction.open_table(RECEIPTS).map_err(refused)?;
            if let Some(receipt) = receipt {
                let bytes = serde_json::to_vec(receipt).map_err(refused)?;
                table
                    .insert(receipt.receipt_id.as_u64(), bytes.as_slice())
                    .map_err(refused)?;
            }
            for id in removals {
                table.remove(id.as_u64()).map_err(refused)?;
            }
        }
        transaction.commit().map_err(refused)
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
            if self.closed || self.receiver.changed().await.is_err() {
                return Err(QuerySourceClosed);
            }
            Ok(self.receiver.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {
        self.closed = true;
    }
}

fn initialize(database: &Database) -> Result<(), WriteStoreError> {
    let mut transaction = database.begin_write().map_err(refused)?;
    transaction
        .set_durability(Durability::Immediate)
        .map_err(refused)?;
    {
        transaction.open_table(RECEIPTS).map_err(refused)?;
        let mut meta = transaction.open_table(META).map_err(refused)?;
        if meta.get(NEXT_ID).map_err(refused)?.is_none() {
            meta.insert(NEXT_ID, 1_u64).map_err(refused)?;
        }
    }
    transaction.commit().map_err(refused)
}

fn load(database: &Database) -> Result<(u64, BTreeMap<ReceiptId, Receipt>), WriteStoreError> {
    let transaction = database.begin_read().map_err(refused)?;
    let next_identity = transaction
        .open_table(META)
        .map_err(refused)?
        .get(NEXT_ID)
        .map_err(refused)?
        .ok_or_else(|| WriteStoreError::Refused("write identity metadata missing".to_owned()))?
        .value();
    let table = transaction.open_table(RECEIPTS).map_err(refused)?;
    let mut receipts = BTreeMap::new();
    for entry in table.iter().map_err(refused)? {
        let (_, value) = entry.map_err(refused)?;
        let receipt: Receipt = serde_json::from_slice(value.value()).map_err(refused)?;
        if receipts.insert(receipt.receipt_id, receipt).is_some() {
            return Err(WriteStoreError::Refused(
                "duplicate durable receipt identity".to_owned(),
            ));
        }
    }
    Ok((next_identity, receipts))
}

fn recover_ambiguous(receipts: &mut BTreeMap<ReceiptId, Receipt>) -> Vec<Receipt> {
    let mut recovered = Vec::new();
    for receipt in receipts.values_mut() {
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
            ops::settle(receipt);
            recovered.push(receipt.clone());
        }
    }
    recovered
}

fn persist_many<'a>(
    database: &Database,
    receipts: impl IntoIterator<Item = &'a Receipt>,
) -> Result<(), WriteStoreError> {
    let mut transaction = database.begin_write().map_err(refused)?;
    transaction
        .set_durability(Durability::Immediate)
        .map_err(refused)?;
    {
        let mut table = transaction.open_table(RECEIPTS).map_err(refused)?;
        for receipt in receipts {
            let bytes = serde_json::to_vec(receipt).map_err(refused)?;
            table
                .insert(receipt.receipt_id.as_u64(), bytes.as_slice())
                .map_err(refused)?;
        }
    }
    transaction.commit().map_err(refused)
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
    }
}

fn refused(error: impl std::fmt::Display) -> WriteStoreError {
    WriteStoreError::Refused(error.to_string())
}
