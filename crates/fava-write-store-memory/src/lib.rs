//! Bounded volatile write-store provider for tests and explicit ephemeral profiles.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError, SourceChangeFuture,
    SourceChanges, SourceEvent, SourceKind, SourceRevision, SourceSnapshot, SourceStatus,
};
use fava_write::{
    EventValue, LocalWriteEvent, PublicationEvidence, ReceiptId, SignatureState, WriteId,
};
use fava_write_store::{AcceptedWrite, WriteStore, WriteStoreError};
use tokio::sync::watch;

/// Bounded current-process write store.
pub struct MemoryWriteStore {
    capacity: NonZeroUsize,
    state: Mutex<WriteState>,
    latest: watch::Sender<Arc<SourceSnapshot>>,
}

#[derive(Clone, Debug)]
struct WriteState {
    revision: u64,
    next_identity: u64,
    writes: BTreeMap<ReceiptId, LocalWriteEvent>,
}

impl Default for WriteState {
    fn default() -> Self {
        Self {
            revision: 0,
            next_identity: 1,
            writes: BTreeMap::new(),
        }
    }
}

impl Default for MemoryWriteStore {
    fn default() -> Self {
        Self::bounded(NonZeroUsize::new(10_000).expect("constant is non-zero"))
    }
}

impl MemoryWriteStore {
    /// Create an empty store with an exact maximum active-write count.
    #[must_use]
    pub fn bounded(capacity: NonZeroUsize) -> Self {
        let (latest, _) = watch::channel(Arc::new(SourceSnapshot::empty(SourceKind::WriteStore)));
        Self {
            capacity,
            state: Mutex::new(WriteState::default()),
            latest,
        }
    }

    fn snapshot(state: &WriteState) -> SourceSnapshot {
        SourceSnapshot {
            kind: SourceKind::WriteStore,
            revision: SourceRevision(state.revision),
            status: SourceStatus::Open,
            events: state
                .writes
                .values()
                .cloned()
                .map(SourceEvent::Local)
                .collect(),
        }
    }

    fn publish_snapshot(&self, state: &WriteState) {
        self.latest.send_replace(Arc::new(Self::snapshot(state)));
    }
}

impl WriteStore for MemoryWriteStore {
    fn accept_materialized(&self, event: EventValue) -> Result<AcceptedWrite, WriteStoreError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        if guard.writes.len() == self.capacity.get() {
            return Err(WriteStoreError::Refused(format!(
                "bounded write-store capacity {} reached",
                self.capacity
            )));
        }

        let identity = guard.next_identity;
        let next_identity = identity
            .checked_add(1)
            .ok_or_else(|| WriteStoreError::Refused("write identity exhausted".to_owned()))?;
        let write_id = WriteId::from_u64(identity);
        let receipt_id = ReceiptId::from_u64(identity);
        let signature = match &event {
            EventValue::Unsigned(_) => SignatureState::Unsigned,
            EventValue::Signed(_) => SignatureState::Signed,
        };
        let publication = PublicationEvidence {
            receipt_id,
            write_id,
            signature,
        };
        let current = LocalWriteEvent::new(event, publication)?;

        guard.next_identity = next_identity;
        guard.revision = guard
            .revision
            .checked_add(1)
            .ok_or_else(|| WriteStoreError::Refused("source revision exhausted".to_owned()))?;
        guard.writes.insert(receipt_id, current.clone());
        self.publish_snapshot(&guard);

        Ok(AcceptedWrite {
            write_id,
            receipt_id,
            current,
        })
    }

    fn cancel(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        let removed = guard.writes.remove(&receipt_id).is_some();
        if removed {
            guard.revision = guard
                .revision
                .checked_add(1)
                .ok_or_else(|| WriteStoreError::Refused("source revision exhausted".to_owned()))?;
            self.publish_snapshot(&guard);
        }
        Ok(removed)
    }

    fn receipt_event(
        &self,
        receipt_id: ReceiptId,
    ) -> Result<Option<LocalWriteEvent>, WriteStoreError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        Ok(guard.writes.get(&receipt_id).cloned())
    }

    fn len(&self) -> Result<usize, WriteStoreError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        Ok(guard.writes.len())
    }
}

impl QuerySource for MemoryWriteStore {
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
