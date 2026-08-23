//! One consumer's bounded view of a fake session's inbound items.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava_query::OperationGeneration;
use fava_state::RelaySessionKey;
use fava_transport::{
    RelayInbound, RelayInboundFuture, RelayMessageStream, RelaySessionIdentity, TransportError,
    TransportFailure,
};
use tokio::sync::Notify;

/// Live generation of one fake session, readable without the session lock.
pub(crate) struct LiveIdentity {
    pub(crate) key: RelaySessionKey,
    pub(crate) generation: AtomicU64,
}

impl LiveIdentity {
    pub(crate) fn read(&self) -> RelaySessionIdentity {
        RelaySessionIdentity {
            key: self.key.clone(),
            generation: OperationGeneration(self.generation.load(Ordering::SeqCst)),
        }
    }
}

/// Bounded per-consumer queue. One consumer can never take another's item.
pub(crate) struct ConsumerState {
    capacity: usize,
    items: Mutex<VecDeque<RelayInbound>>,
    dropped: AtomicU64,
    detached: AtomicBool,
    notify: Notify,
}

impl ConsumerState {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: Mutex::new(VecDeque::new()),
            dropped: AtomicU64::new(0),
            detached: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }

    /// Offer one item. A full queue records exact loss instead of parking the
    /// producer or silently discarding (GOALS:434, GOALS:1448).
    pub(crate) fn offer(&self, item: RelayInbound) {
        if self.detached.load(Ordering::SeqCst) {
            return;
        }
        {
            let mut items = self.items.lock().expect("consumer queue is not poisoned");
            if items.len() >= self.capacity {
                self.dropped.fetch_add(1, Ordering::SeqCst);
            } else {
                items.push_back(item);
            }
        }
        self.notify.notify_waiters();
    }

    pub(crate) fn detach(&self) {
        self.detached.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub(crate) fn is_detached(&self) -> bool {
        self.detached.load(Ordering::SeqCst)
    }

    fn take(&self, identity: &RelaySessionIdentity) -> Option<RelayInbound> {
        let mut items = self.items.lock().expect("consumer queue is not poisoned");
        if let Some(item) = items.pop_front() {
            return Some(item);
        }
        let dropped = self.dropped.swap(0, Ordering::SeqCst);
        (dropped > 0).then(|| RelayInbound::Lost {
            identity: identity.clone(),
            dropped,
        })
    }
}

/// A fake session's inbound stream for exactly one consumer.
pub(crate) struct FakeMessageStream {
    pub(crate) consumer: Arc<ConsumerState>,
    pub(crate) identity: Arc<LiveIdentity>,
    pub(crate) idle: Duration,
}

impl RelayMessageStream for FakeMessageStream {
    fn next_inbound(&mut self) -> RelayInboundFuture<'_> {
        Box::pin(async move {
            loop {
                let notified = self.consumer.notify.notified();
                let identity = self.identity.read();
                if let Some(item) = self.consumer.take(&identity) {
                    return Ok(item);
                }
                if self.consumer.is_detached() {
                    return Err(TransportError::Closed(identity.clone()));
                }
                if tokio::time::timeout(self.idle, notified).await.is_err() {
                    self.consumer.detach();
                    return Ok(RelayInbound::Disconnected {
                        identity,
                        reason: TransportFailure::IdleTimeout { after: self.idle },
                    });
                }
            }
        })
    }

    fn close(&mut self) {
        self.consumer.detach();
    }
}

impl Drop for FakeMessageStream {
    fn drop(&mut self) {
        self.consumer.detach();
    }
}
