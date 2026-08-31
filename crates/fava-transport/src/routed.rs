//! What one relay session delivers to the component that asked for it.
//!
//! A handle is minted by the verb that produced its wire key: `req` yields a
//! [`Subscription`], `event` and `auth` yield an [`Acknowledgement`]. Each
//! delivers its own narrow item type, so a consumer never writes an arm for a
//! message that cannot reach it.

use std::future::Future;
use std::pin::Pin;

use fava_wire::{RelayMessage, SubscriptionId};
use nostr::event::{Event, EventId};

use crate::BoundedText;

/// Why a handle's connection ended.
///
/// A publication attempt must report which of these happened rather than
/// collapsing them into one unknown outcome (`.planning/REQUIREMENTS.md`
/// HARD-07).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionEnded {
    /// The connection dropped. A reconnect may follow.
    Disconnected {
        /// Exact scoped reason.
        detail: BoundedText,
    },
    /// The reconnect budget is spent; no further connection will appear.
    ReconnectExhausted {
        /// Number of attempts actually made.
        attempts: usize,
        /// Exact reason of the final attempt.
        detail: BoundedText,
    },
}

/// A change in the state of one session's connection.
///
/// Any lease holder can read these, whether or not it holds a subscription or
/// an outstanding acknowledgement: what a component proved to the relay, and
/// what it parked awaiting an answer, belong to the connection that carried
/// them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    /// The connection dropped. A reconnect may follow.
    Disconnected {
        /// Exact scoped reason.
        detail: BoundedText,
    },
    /// A new connection is live. Every holder MUST replay its demand.
    Reconnected {
        /// The connection now current.
        identity: crate::RelaySessionIdentity,
    },
    /// The reconnect budget is spent; no further connection will appear.
    Unreachable {
        /// Number of attempts actually made.
        attempts: usize,
        /// Exact reason of the final attempt.
        detail: BoundedText,
    },
}

/// One item delivered to one live subscription.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubscriptionItem {
    /// One event the relay attributed to this subscription.
    Event(Box<Event>),
    /// The relay has sent everything it stored for this subscription.
    EndOfStoredEvents,
    /// The relay refused or ended this subscription, with its own text.
    Closed {
        /// Verbatim, bounded relay text.
        reason: BoundedText,
    },
    /// This subscription's bounded queue overflowed. Loss is typed, never
    /// silent (GOALS:434, QUERY-011).
    Lost {
        /// Exact number of items dropped since the last `Lost`.
        dropped: u64,
    },
    /// The connection this subscription was opened on ended. Nothing further
    /// arrives; the demand must be replayed on the next connection.
    Ended(SessionEnded),
}

/// How one publication or authentication attempt settled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Settlement {
    /// The relay accepted the event, with its own text.
    Accepted {
        /// Verbatim, bounded relay text.
        message: BoundedText,
    },
    /// The relay rejected the event, with its own text.
    Rejected {
        /// Verbatim, bounded relay text.
        message: BoundedText,
    },
    /// The connection the event was handed off on ended before the relay
    /// answered. The outcome is unknown, never a rejection.
    Ended(SessionEnded),
}

/// One live wire subscription, and the only way to read what it carries.
///
/// Dropping the handle releases its wire key and sends the relay's `CLOSE`, so
/// a subscription cannot stop being read while the relay is still sending it.
pub trait Subscription: Send {
    /// Wire identifier the session minted for this subscription.
    fn id(&self) -> &SubscriptionId;

    /// Await the next item for this subscription alone.
    fn next(&mut self) -> SubscriptionFuture<'_>;

    /// Send the relay's `CLOSE` and report that frame's handoff outcome.
    ///
    /// Dropping the handle instead enqueues the same `CLOSE` without waiting
    /// for its outcome.
    fn close(self: Box<Self>) -> CloseFuture;
}

/// One outstanding relay acknowledgement.
pub trait Acknowledgement: Send {
    /// Event identifier whose `OK` this handle is awaiting.
    fn event_id(&self) -> EventId;

    /// Await the relay's verdict on this event alone.
    ///
    /// Takes `&mut self` rather than consuming the handle so a caller can wait
    /// on it alongside something else -- a publication watching for an
    /// authentication challenge, say -- without giving it up to do so.
    fn settled(&mut self) -> SettlementFuture<'_>;
}

/// What a decoded relay message correlates to, if anything.
///
/// The mapping is fixed: subscription-correlated messages by subscription id,
/// `OK` by event id, `AUTH` to the challenge reader, and everything else to
/// nobody.
#[must_use]
pub fn correlation(message: &RelayMessage<'_>) -> Correlation {
    match message {
        RelayMessage::Event {
            subscription_id, ..
        }
        | RelayMessage::EndOfStoredEvents(subscription_id)
        | RelayMessage::Closed {
            subscription_id, ..
        }
        | RelayMessage::Count {
            subscription_id, ..
        }
        | RelayMessage::NegMsg {
            subscription_id, ..
        }
        | RelayMessage::NegErr {
            subscription_id, ..
        } => Correlation::Subscription(subscription_id.as_ref().clone()),
        RelayMessage::Ok { event_id, .. } => Correlation::Acknowledgement(*event_id),
        RelayMessage::Auth { .. } => Correlation::Challenge,
        RelayMessage::Notice(_) => Correlation::Unclaimed,
    }
}

/// The wire key a decoded relay message belongs to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Correlation {
    /// Belongs to the handle holding this subscription id.
    Subscription(SubscriptionId),
    /// Belongs to every handle awaiting this event's verdict.
    Acknowledgement(EventId),
    /// Belongs to the session's challenge reader.
    Challenge,
    /// Belongs to nobody. Counted, never delivered as another's work.
    Unclaimed,
}

/// Future yielding one subscription item.
pub type SubscriptionFuture<'a> = Pin<Box<dyn Future<Output = SubscriptionItem> + Send + 'a>>;

/// Future yielding one settlement.
pub type SettlementFuture<'a> = Pin<Box<dyn Future<Output = Settlement> + Send + 'a>>;

/// Future yielding the handoff outcome of one `CLOSE`.
pub type CloseFuture = Pin<Box<dyn Future<Output = crate::HandoffOutcome> + Send>>;

/// A subscription reading from a [`crate::Mailbox`] on a session's router.
///
/// Every transport shares this: the implementation owns the socket, the router
/// decides where a message goes, and this reads one handle's queue.
pub struct RoutedSubscription {
    id: SubscriptionId,
    mailbox: std::sync::Arc<crate::Mailbox<SubscriptionItem>>,
    session: std::sync::Arc<dyn crate::RelaySession>,
    /// Cleared by `close`, so `Drop` does not send a second `CLOSE`.
    armed: bool,
}

impl RoutedSubscription {
    /// Bind one minted identifier to the mailbox its messages land in.
    pub fn new(
        id: SubscriptionId,
        mailbox: std::sync::Arc<crate::Mailbox<SubscriptionItem>>,
        session: std::sync::Arc<dyn crate::RelaySession>,
    ) -> Self {
        Self {
            id,
            mailbox,
            session,
            armed: true,
        }
    }

    fn close_frame(&self) -> Option<Vec<u8>> {
        fava_wire::encode_client(&fava_wire::ClientMessage::close(self.id.clone()))
            .ok()
            .map(String::into_bytes)
    }
}

impl Subscription for RoutedSubscription {
    fn id(&self) -> &SubscriptionId {
        &self.id
    }

    fn next(&mut self) -> SubscriptionFuture<'_> {
        Box::pin(async move {
            loop {
                let notified = self.mailbox.notified();
                if let Some(item) = self.mailbox.take() {
                    return item;
                }
                let dropped = self.mailbox.take_dropped();
                if dropped > 0 {
                    return SubscriptionItem::Lost { dropped };
                }
                if self.mailbox.is_closed() {
                    return SubscriptionItem::Ended(SessionEnded::Disconnected {
                        detail: BoundedText::new("session closed"),
                    });
                }
                notified.await;
            }
        })
    }

    fn close(mut self: Box<Self>) -> CloseFuture {
        self.armed = false;
        Box::pin(async move {
            self.session.router().release_subscription(&self.id);
            let Some(frame) = self.close_frame() else {
                return crate::HandoffOutcome::NotHandedOff {
                    identity: self.session.identity(),
                    correlation: crate::HandoffCorrelation::new(0),
                    reason: crate::TransportFailure::SessionClosed,
                };
            };
            self.session
                .hand_off(frame, crate::HandoffCorrelation::new(0))
                .await
        })
    }
}

impl Drop for RoutedSubscription {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.session.router().release_subscription(&self.id);
        // A handle released without closing still tells the relay, so no
        // subscription is left streaming to a component that stopped reading.
        if let Some(frame) = self.close_frame() {
            self.session.enqueue(frame);
        }
    }
}

/// An acknowledgement reading from a [`crate::Mailbox`] on a session's router.
pub struct RoutedAcknowledgement {
    event: EventId,
    mailbox: std::sync::Arc<crate::Mailbox<Settlement>>,
    session: std::sync::Arc<dyn crate::RelaySession>,
}

impl RoutedAcknowledgement {
    /// Bind one in-flight event to the mailbox its verdict lands in.
    pub fn new(
        event: EventId,
        mailbox: std::sync::Arc<crate::Mailbox<Settlement>>,
        session: std::sync::Arc<dyn crate::RelaySession>,
    ) -> Self {
        Self {
            event,
            mailbox,
            session,
        }
    }
}

impl Acknowledgement for RoutedAcknowledgement {
    fn event_id(&self) -> EventId {
        self.event
    }

    fn settled(&mut self) -> SettlementFuture<'_> {
        Box::pin(async move {
            loop {
                let notified = self.mailbox.notified();
                if let Some(settlement) = self.mailbox.take() {
                    self.session
                        .router()
                        .release_acknowledgement(self.event, &self.mailbox);
                    return settlement;
                }
                if self.mailbox.is_closed() {
                    self.session
                        .router()
                        .release_acknowledgement(self.event, &self.mailbox);
                    return Settlement::Ended(SessionEnded::Disconnected {
                        detail: BoundedText::new("session closed"),
                    });
                }
                notified.await;
            }
        })
    }
}

/// The verbs that yield a handle.
///
/// These live on `Arc<dyn RelaySession>` rather than on the trait itself
/// because a handle outlives the call that made it and must keep the session
/// alive; `&self` cannot produce that.
pub trait RelaySessionExt {
    /// Open one wire subscription carrying `filters`, and read what it carries.
    ///
    /// The session names the subscription; the caller supplies only what to
    /// match.
    fn subscribe(&self, filters: Vec<nostr::filter::Filter>) -> SubscribeFuture<'_>;

    /// Publish one signed event and await the relay's verdict on it alone.
    fn publish(&self, event: nostr::event::Event) -> PublishFuture<'_>;

    /// Answer one NIP-42 challenge and await the relay's verdict on it alone.
    fn answer(&self, event: nostr::event::Event) -> PublishFuture<'_>;

    /// Read this session's connection state changes.
    ///
    /// This is a fact about the session, not a footnote on some other stream:
    /// a lease holder that owns no wire key still needs it.
    fn connection(&self) -> std::sync::Arc<crate::Mailbox<ConnectionState>>;

    /// Read the relay's authentication challenges.
    ///
    /// A challenge is correlated to nothing a component sent, and exactly one
    /// component owns it, so it has its own reader rather than a claim.
    fn challenges(&self) -> std::sync::Arc<crate::Mailbox<String>>;
}

/// Outcome of opening one subscription: the handle, or why no frame left.
pub type SubscribeFuture<'a> = std::pin::Pin<
    Box<dyn Future<Output = Result<Box<dyn Subscription>, crate::HandoffOutcome>> + Send + 'a>,
>;

/// Outcome of publishing one event: the handle, or why no frame left.
pub type PublishFuture<'a> = std::pin::Pin<
    Box<dyn Future<Output = Result<Box<dyn Acknowledgement>, crate::HandoffOutcome>> + Send + 'a>,
>;

impl RelaySessionExt for std::sync::Arc<dyn crate::RelaySession> {
    fn subscribe(&self, filters: Vec<nostr::filter::Filter>) -> SubscribeFuture<'_> {
        Box::pin(async move {
            let id = self.mint_subscription_id();
            // Register before sending: the reader and the writer share one
            // task, but the mailbox must exist before the relay can answer.
            let mailbox = self
                .router()
                .open_subscription(id.clone(), self.inbound_capacity());
            let message = fava_wire::ClientMessage::Req {
                subscription_id: std::borrow::Cow::Owned(id.clone()),
                filters: filters.into_iter().map(std::borrow::Cow::Owned).collect(),
            };
            match self.encoded(&message).await {
                crate::HandoffOutcome::HandedOff { .. } => Ok(Box::new(RoutedSubscription::new(
                    id,
                    mailbox,
                    std::sync::Arc::clone(self),
                ))
                    as Box<dyn Subscription>),
                refused => {
                    self.router().release_subscription(&id);
                    Err(refused)
                }
            }
        })
    }

    fn publish(&self, event: nostr::event::Event) -> PublishFuture<'_> {
        Box::pin(async move {
            let id = event.id;
            let mailbox = self.router().await_acknowledgement(id);
            match self.encoded(&fava_wire::ClientMessage::event(event)).await {
                crate::HandoffOutcome::HandedOff { .. } => Ok(Box::new(RoutedAcknowledgement::new(
                    id,
                    mailbox,
                    std::sync::Arc::clone(self),
                ))
                    as Box<dyn Acknowledgement>),
                refused => {
                    self.router().release_acknowledgement(id, &mailbox);
                    Err(refused)
                }
            }
        })
    }

    fn answer(&self, event: nostr::event::Event) -> PublishFuture<'_> {
        Box::pin(async move {
            let id = event.id;
            let mailbox = self.router().await_acknowledgement(id);
            match self.encoded(&fava_wire::ClientMessage::auth(event)).await {
                crate::HandoffOutcome::HandedOff { .. } => Ok(Box::new(RoutedAcknowledgement::new(
                    id,
                    mailbox,
                    std::sync::Arc::clone(self),
                ))
                    as Box<dyn Acknowledgement>),
                refused => {
                    self.router().release_acknowledgement(id, &mailbox);
                    Err(refused)
                }
            }
        })
    }

    fn challenges(&self) -> std::sync::Arc<crate::Mailbox<String>> {
        self.router().read_challenges(self.inbound_capacity())
    }

    fn connection(&self) -> std::sync::Arc<crate::Mailbox<ConnectionState>> {
        self.router().read_connection(self.inbound_capacity())
    }
}
