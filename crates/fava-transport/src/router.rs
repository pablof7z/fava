//! Delivery of decoded relay messages to the handle that owns each wire key.
//!
//! The socket reader decodes each frame exactly once and asks
//! [`fava_transport::correlation`] which wire key it belongs to. A message no
//! live handle claims is counted, never delivered as another component's work.

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use fava_wire::{RelayMessage, SubscriptionId};

use crate::BoundedText;
use crate::routed::{Correlation, SessionEnded, Settlement, SubscriptionItem, correlation};
use nostr::event::EventId;
use tokio::sync::Notify;

/// One handle's bounded queue. A full queue records exact loss rather than
/// parking the socket reader or silently discarding (GOALS:434, GOALS:1448).
pub struct Mailbox<T> {
    capacity: usize,
    items: Mutex<VecDeque<T>>,
    dropped: AtomicU64,
    closed: AtomicBool,
    notify: Notify,
}

impl<T> Mailbox<T> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: Mutex::new(VecDeque::new()),
            dropped: AtomicU64::new(0),
            closed: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }

    fn offer(&self, item: T) {
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        {
            let mut items = self.items.lock().expect("mailbox is not poisoned");
            if items.len() >= self.capacity {
                self.dropped.fetch_add(1, Ordering::SeqCst);
            } else {
                items.push_back(item);
            }
        }
        self.notify.notify_waiters();
    }

    /// See the type's documentation.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn take(&self) -> Option<T> {
        self.items
            .lock()
            .expect("mailbox is not poisoned")
            .pop_front()
    }

    /// Exact items dropped since the last call, then reset.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn take_dropped(&self) -> u64 {
        self.dropped.swap(0, Ordering::SeqCst)
    }

    /// See the type's documentation.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// See the type's documentation.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    /// See the type's documentation.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.notify.notified()
    }
}

/// Counts of traffic that reached no handle. These are the only signal that a
/// component sent a request outside the verbs, which would otherwise present
/// as a silent hang.
#[derive(Default)]
pub struct Unrouted {
    /// Decoded messages no live handle's wire key claimed.
    pub unclaimed: AtomicU64,
    /// Inbound bytes `fava-wire` refused.
    pub undecodable: AtomicU64,
}

/// Every live handle on one session, keyed by the wire key it owns.
#[derive(Default)]
pub struct Router {
    subscriptions: Mutex<BTreeMap<SubscriptionId, Arc<Mailbox<SubscriptionItem>>>>,
    /// An acknowledgement fans out: two callers publishing one event both want
    /// the relay's single verdict.
    acknowledgements: Mutex<BTreeMap<EventId, Vec<Arc<Mailbox<Settlement>>>>>,
    challenges: Mutex<Vec<Arc<Mailbox<BoundedText>>>>,
    /// Traffic that reached no handle.
    pub unrouted: Unrouted,
}

impl Router {
    /// See the type's documentation.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn open_subscription(
        &self,
        id: SubscriptionId,
        capacity: usize,
    ) -> Arc<Mailbox<SubscriptionItem>> {
        let mailbox = Arc::new(Mailbox::new(capacity));
        self.subscriptions
            .lock()
            .expect("router is not poisoned")
            .insert(id, Arc::clone(&mailbox));
        mailbox
    }

    /// See the type's documentation.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn release_subscription(&self, id: &SubscriptionId) {
        if let Some(mailbox) = self
            .subscriptions
            .lock()
            .expect("router is not poisoned")
            .remove(id)
        {
            mailbox.close();
        }
    }

    /// See the type's documentation.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn await_acknowledgement(&self, event: EventId) -> Arc<Mailbox<Settlement>> {
        let mailbox = Arc::new(Mailbox::new(1));
        self.acknowledgements
            .lock()
            .expect("router is not poisoned")
            .entry(event)
            .or_default()
            .push(Arc::clone(&mailbox));
        mailbox
    }

    /// See the type's documentation.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn release_acknowledgement(&self, event: EventId, mailbox: &Arc<Mailbox<Settlement>>) {
        let mut held = self.acknowledgements.lock().expect("router is not poisoned");
        if let Some(waiting) = held.get_mut(&event) {
            waiting.retain(|existing| !Arc::ptr_eq(existing, mailbox));
            if waiting.is_empty() {
                held.remove(&event);
            }
        }
    }

    /// See the type's documentation.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn read_challenges(&self, capacity: usize) -> Arc<Mailbox<BoundedText>> {
        let mailbox = Arc::new(Mailbox::new(capacity));
        self.challenges
            .lock()
            .expect("router is not poisoned")
            .push(Arc::clone(&mailbox));
        mailbox
    }

    /// Deliver one decoded message to whatever owns its wire key.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn deliver(&self, message: RelayMessage<'static>) {
        match correlation(&message) {
            Correlation::Subscription(id) => {
                let held = self.subscriptions.lock().expect("router is not poisoned");
                let Some(mailbox) = held.get(&id) else {
                    drop(held);
                    self.unrouted.unclaimed.fetch_add(1, Ordering::SeqCst);
                    return;
                };
                let mailbox: Arc<Mailbox<SubscriptionItem>> = Arc::clone(mailbox);
                drop(held);
                mailbox.offer(subscription_item(message));
            }
            Correlation::Acknowledgement(event) => {
                let held = self.acknowledgements.lock().expect("router is not poisoned");
                let Some(waiting) = held.get(&event) else {
                    drop(held);
                    self.unrouted.unclaimed.fetch_add(1, Ordering::SeqCst);
                    return;
                };
                let waiting: Vec<_> = waiting.iter().map(Arc::clone).collect();
                drop(held);
                let settlement = settlement(&message);
                for mailbox in waiting {
                    mailbox.offer(settlement.clone());
                }
            }
            Correlation::Challenge => {
                let RelayMessage::Auth { challenge } = &message else {
                    return;
                };
                let readers: Vec<_> = self
                    .challenges
                    .lock()
                    .expect("router is not poisoned")
                    .iter()
                    .map(Arc::clone)
                    .collect();
                if readers.is_empty() {
                    self.unrouted.unclaimed.fetch_add(1, Ordering::SeqCst);
                    return;
                }
                for mailbox in readers {
                    mailbox.offer(BoundedText::new(challenge.as_ref()));
                }
            }
            Correlation::Unclaimed => {
                self.unrouted.unclaimed.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    /// Bytes the relay sent that `fava-wire` refused. Counted, never delivered.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn undecodable(&self) {
        self.unrouted.undecodable.fetch_add(1, Ordering::SeqCst);
    }

    /// End every live handle, because the wire state they name did not survive
    /// the connection. A released handle reports the ending rather than waiting.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn end_generation(&self, ended: &SessionEnded) {
        for (_, mailbox) in std::mem::take(
            &mut *self
                .subscriptions
                .lock()
                .expect("router is not poisoned"),
        ) {
            mailbox.offer(SubscriptionItem::Ended(ended.clone()));
            mailbox.close();
        }
        for (_, waiting) in std::mem::take(
            &mut *self
                .acknowledgements
                .lock()
                .expect("router is not poisoned"),
        ) {
            for mailbox in waiting {
                mailbox.offer(Settlement::Ended(ended.clone()));
                mailbox.close();
            }
        }
    }

    /// Whether any wire key is still held. The router retains nothing across a
    /// generation, and this is how a test proves it.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn retained(&self) -> usize {
        self.subscriptions
            .lock()
            .expect("router is not poisoned")
            .len()
            + self
                .acknowledgements
                .lock()
                .expect("router is not poisoned")
                .len()
    }
}

fn subscription_item(message: RelayMessage<'static>) -> SubscriptionItem {
    match message {
        RelayMessage::Event { event, .. } => SubscriptionItem::Event(Box::new(event.into_owned())),
        RelayMessage::EndOfStoredEvents(_) => SubscriptionItem::EndOfStoredEvents,
        RelayMessage::Closed { message, .. } => SubscriptionItem::Closed {
            reason: BoundedText::new(message),
        },
        // COUNT and the negentropy messages correlate to a subscription but
        // carry nothing this contract exposes yet; they are counted as closed
        // out rather than invented into an event.
        other => SubscriptionItem::Closed {
            reason: BoundedText::new(format!("unsupported relay message: {other:?}")),
        },
    }
}

fn settlement(message: &RelayMessage<'_>) -> Settlement {
    match message {
        RelayMessage::Ok {
            status: true,
            message,
            ..
        } => Settlement::Accepted {
            message: BoundedText::new(message.as_ref()),
        },
        RelayMessage::Ok {
            status: false,
            message,
            ..
        } => Settlement::Rejected {
            message: BoundedText::new(message.as_ref()),
        },
        other => Settlement::Rejected {
            message: BoundedText::new(format!("unexpected acknowledgement: {other:?}")),
        },
    }
}
