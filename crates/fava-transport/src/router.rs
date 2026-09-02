//! Delivery of decoded relay messages to the handle that owns each wire key.
//!
//! The socket reader decodes each frame exactly once and asks
//! [`fava_transport::correlation`] which wire key it belongs to. A message no
//! live handle claims is counted, never delivered as another component's work.

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use fava_wire::{RelayMessage, SubscriptionId};

use crate::BoundedText;
use crate::routed::{Correlation, SessionEnded, Settlement, SubscriptionItem, correlation};
use crate::session::Connection;
use fava_relay::{Authentication, Connectivity};
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
pub struct Router {
    subscriptions: Mutex<BTreeMap<SubscriptionId, Arc<Mailbox<SubscriptionItem>>>>,
    /// An acknowledgement fans out: two callers publishing one event both want
    /// the relay's single verdict.
    acknowledgements: Mutex<BTreeMap<EventId, Vec<Arc<Mailbox<Settlement>>>>>,
    /// Readers of the relay's authentication challenges.
    ///
    /// The text is carried verbatim, not bounded: the one component that reads
    /// challenges refuses an oversized one rather than truncating it, and a
    /// bound applied here would silently defeat that. The frame it arrived in
    /// is already bounded by `max_frame_bytes`, so this cannot grow unbounded.
    challenges: Mutex<Vec<Arc<Mailbox<String>>>>,
    /// Where this connection has got to.
    ///
    /// A current value rather than a queue of past ones: a reader arriving
    /// late wants the state as it is, and a reader falling behind wants the
    /// newest rather than the oldest. Dropping the sender is how a connection
    /// says it will never reach anything again; `close` is what drops it.
    connection: Mutex<ConnectionCell>,
    /// Traffic that reached no handle.
    pub unrouted: Unrouted,
}

/// The connection watch, plus the last value it carried once the sender
/// behind it is gone.
///
/// Held separately from the sender because a `watch::Sender` cannot be asked
/// for its value once dropped, and a reader arriving after close still needs
/// to see where the connection ended rather than nothing at all.
struct ConnectionCell {
    sender: Option<watch::Sender<Connection>>,
    last: Connection,
}

impl Router {
    /// A router for one connection, in the state it starts in.
    #[must_use]
    pub fn new(connection: Connection) -> Self {
        Self {
            subscriptions: Mutex::default(),
            acknowledgements: Mutex::default(),
            challenges: Mutex::default(),
            connection: Mutex::new(ConnectionCell {
                last: connection.clone(),
                sender: Some(watch::Sender::new(connection)),
            }),
            unrouted: Unrouted::default(),
        }
    }

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
        let mut held = self
            .acknowledgements
            .lock()
            .expect("router is not poisoned");
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
    /// Watch where this connection has got to.
    ///
    /// The receiver holds the current value, so a reader that arrives after a
    /// change still sees it. A reader that arrives after `close` gets a
    /// receiver over the connection's final value whose `changed` fails
    /// immediately, rather than one that waits on a sender no longer there.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    #[must_use]
    pub fn connection(&self) -> watch::Receiver<Connection> {
        let cell = self.connection.lock().expect("router is not poisoned");
        if let Some(sender) = &cell.sender {
            sender.subscribe()
        } else {
            let (sender, receiver) = watch::channel(cell.last.clone());
            drop(sender);
            receiver
        }
    }

    /// Move this connection to a new state.
    ///
    /// One write per transition, and the handles follow from it. Every wire
    /// key names state the connection carried, so a connection that drops or
    /// is replaced ends them rather than leaving them waiting on a relay that
    /// has forgotten them. The caller says where the connection got to; what
    /// that means for the handles is not a second decision.
    ///
    /// A no-op once the sender is gone: nothing here reaches a closed
    /// connection, because nothing about it can still change.
    ///
    /// A transition that lands on `Connectivity::Disconnected { spent: Some(_), .. }`
    /// drops the sender before returning: that is the terminal state, whether
    /// it was reached by an exhausted reconnect budget or by a deliberate
    /// close, and a watcher told to wait must learn instead that nothing here
    /// will ever change again, not go on waiting for a write that will not
    /// come.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn moved(&self, next: impl FnOnce(&mut Connection)) {
        let ended = {
            let mut ended = None;
            let mut cell = self.connection.lock().expect("router is not poisoned");
            let Some(sender) = &cell.sender else {
                return;
            };
            sender.send_modify(|connection| {
                let was = connection.identity.clone();
                let live = matches!(connection.connectivity, Connectivity::Connected);
                next(connection);
                ended = match &connection.connectivity {
                    Connectivity::Disconnected {
                        detail,
                        spent: Some(attempts),
                    } => Some(SessionEnded::ReconnectExhausted {
                        attempts: *attempts,
                        detail: detail.clone(),
                    }),
                    Connectivity::Disconnected { detail, .. } => Some(SessionEnded::Disconnected {
                        detail: detail.clone(),
                    }),
                    // A replacement carries none of what the previous one held.
                    _ if connection.identity != was && live => Some(SessionEnded::Disconnected {
                        detail: BoundedText::new("session reconnected"),
                    }),
                    _ => None,
                };
            });
            let current = sender.borrow().clone();
            let terminal = matches!(
                current.connectivity,
                Connectivity::Disconnected { spent: Some(_), .. }
            );
            cell.last = current;
            if terminal {
                cell.sender = None;
            }
            ended
        };
        if let Some(ended) = ended {
            self.end_connection(&ended);
        }
    }

    /// Close the connection watch: write a terminal `Disconnected` state
    /// (unless it already carries a more specific reason to stop), which
    /// `moved` itself turns into dropping the sender.
    fn close_connection(&self) {
        self.moved(|connection| {
            if !matches!(
                connection.connectivity,
                Connectivity::Disconnected { spent: Some(_), .. }
            ) {
                connection.connectivity = Connectivity::Disconnected {
                    detail: BoundedText::new("session closed"),
                    spent: Some(0),
                };
            }
        });
    }

    /// Read the relay's authentication challenges on this session.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn read_challenges(&self, capacity: usize) -> Arc<Mailbox<String>> {
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
                let held = self
                    .acknowledgements
                    .lock()
                    .expect("router is not poisoned");
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
                // A demand is what the connection now is, not a message
                // addressed to somebody. Stated here it cannot outlive the
                // connection it arrived on, and a relay repeating itself is
                // not a change.
                let challenge = challenge.as_ref().to_owned();
                self.moved(|connection| {
                    connection.authentication = Authentication::Requested { challenge };
                });
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
    pub fn end_connection(&self, ended: &SessionEnded) {
        for (_, mailbox) in
            std::mem::take(&mut *self.subscriptions.lock().expect("router is not poisoned"))
        {
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

    /// Close every reader on this session, including the ones that survive a
    /// reset. A session that is gone has no further facts to report, and a
    /// reader left open never learns to stop waiting.
    ///
    /// # Panics
    ///
    /// If a prior holder of this session's router lock panicked.
    pub fn close(&self) {
        self.close_connection();
        for mailbox in self
            .challenges
            .lock()
            .expect("router is not poisoned")
            .drain(..)
        {
            mailbox.close();
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
