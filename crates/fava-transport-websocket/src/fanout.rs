//! Per-consumer bounded inbound views of one live socket.
//!
//! Every live consumer sees every inbound item. One consumer can never remove
//! an item from another's view, which is what makes shared relay work
//! representable at all (`ARCH:1578`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use fava_relay::RelaySessionKey;
use fava_transport::{
    RelayInbound, RelayInboundFuture, RelayMessageStream, RelaySessionGeneration,
    RelaySessionIdentity, TransportError,
};
use tokio::sync::Notify;

/// Live generation of one session, readable without the driver's lock.
pub(crate) struct LiveIdentity {
    key: RelaySessionKey,
    generation: AtomicU64,
}

impl LiveIdentity {
    pub(crate) fn new(key: RelaySessionKey, generation: RelaySessionGeneration) -> Self {
        Self {
            key,
            generation: AtomicU64::new(generation.get()),
        }
    }

    pub(crate) fn read(&self) -> RelaySessionIdentity {
        RelaySessionIdentity {
            key: self.key.clone(),
            generation: RelaySessionGeneration::new(self.generation.load(Ordering::SeqCst))
                .expect("transport generations are non-zero"),
        }
    }

    pub(crate) fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    /// Retire the current generation and return both identities.
    pub(crate) fn advance(
        &self,
        generation: RelaySessionGeneration,
    ) -> (RelaySessionIdentity, RelaySessionIdentity) {
        let previous = self.read();
        self.generation.store(generation.get(), Ordering::SeqCst);
        (previous, self.read())
    }
}

/// One consumer's bounded queue. A full queue records exact loss rather than
/// parking the socket reader or silently discarding (GOALS:434, GOALS:1448).
pub(crate) struct ConsumerState {
    capacity: usize,
    /// Inbound frames buffered for this consumer, never more than `capacity`.
    items: Mutex<VecDeque<RelayInbound>>,
    dropped: AtomicU64,
    detached: AtomicBool,
    notify: Notify,
}

impl ConsumerState {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: Mutex::new(VecDeque::new()),
            dropped: AtomicU64::new(0),
            detached: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }

    fn offer(&self, item: RelayInbound) {
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

    fn detach(&self) {
        self.detached.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
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

/// Every live consumer of one session.
#[derive(Default)]
pub(crate) struct Consumers {
    registered: Mutex<Vec<Arc<ConsumerState>>>,
}

impl Consumers {
    pub(crate) fn register(&self, capacity: usize) -> Arc<ConsumerState> {
        let consumer = Arc::new(ConsumerState::new(capacity));
        let mut registered = self
            .registered
            .lock()
            .expect("consumer registry is not poisoned");
        registered.retain(|existing| !existing.detached.load(Ordering::SeqCst));
        registered.push(Arc::clone(&consumer));
        consumer
    }

    pub(crate) fn fan_out(&self, item: &RelayInbound) {
        let registered = self
            .registered
            .lock()
            .expect("consumer registry is not poisoned");
        for consumer in registered.iter() {
            consumer.offer(item.clone());
        }
    }

    pub(crate) fn detach_all(&self) {
        let registered = self
            .registered
            .lock()
            .expect("consumer registry is not poisoned");
        for consumer in registered.iter() {
            consumer.detach();
        }
    }
}

/// One consumer's stream over a live WebSocket session.
pub(crate) struct WebSocketMessageStream {
    pub(crate) consumer: Arc<ConsumerState>,
    pub(crate) identity: Arc<LiveIdentity>,
}

impl RelayMessageStream for WebSocketMessageStream {
    fn next_inbound(&mut self) -> RelayInboundFuture<'_> {
        Box::pin(async move {
            loop {
                let notified = self.consumer.notify.notified();
                let identity = self.identity.read();
                if let Some(item) = self.consumer.take(&identity) {
                    return Ok(item);
                }
                if self.consumer.detached.load(Ordering::SeqCst) {
                    return Err(TransportError::Closed(identity));
                }
                notified.await;
            }
        })
    }

    fn close(&mut self) {
        self.consumer.detach();
    }
}

impl Drop for WebSocketMessageStream {
    fn drop(&mut self) {
        self.consumer.detach();
    }
}
