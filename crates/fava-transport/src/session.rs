//! One live connection to a relay, and what it delivers.

use fava_wire::{ClientMessage, SubscriptionId, encode_client};
use nostr::event::Event;
use nostr::filter::Filter;
use nostr::types::RelayUrl;

use crate::{HandoffFuture, HandoffOutcome, ReleaseFuture, ReqFuture, TransportFailure};

/// Exact width, in bytes, of every wire subscription identifier Fava mints.
///
/// A namespace prefix and a zero-padded counter, and nothing else: the width is
/// fixed so the encoded length of a `REQ` is derivable before the identifier
/// exists, and it sits well inside the 64 characters NIP-01 obliges every relay
/// to accept, so a Fava identifier is never too long for a conforming relay.
pub const SUBSCRIPTION_ID_BYTES: usize = 21;

/// One wire subscription the session opened, and the outcome of its `REQ`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenedSubscription {
    /// Identifier the session minted and put on the wire.
    pub id: SubscriptionId,
    /// Outcome of handing the `REQ` frame to the relay.
    pub handoff: HandoffOutcome,
}

/// Mint one fixed-width, opaque wire subscription identifier from a counter.
///
/// Exposed so every `RelaySession` implementation produces the same shape, and
/// so `SUBSCRIPTION_ID_BYTES` has exactly one producer to agree with.
#[must_use]
pub fn subscription_id(counter: u64) -> SubscriptionId {
    let id = format!("fava-{counter:016x}");
    debug_assert_eq!(id.len(), SUBSCRIPTION_ID_BYTES);
    SubscriptionId::new(id)
}

/// Exact authority of one live connection to a relay.
///
/// Authority: ARCH:1567-1571 (`fn identity(&self) -> RelaySessionIdentity`),
/// ARCH:1610 "Every inbound frame and handoff completion carries exact session
/// connection and relay-access identity."
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelaySessionIdentity {
    /// Exact normalized relay URL.
    pub relay: RelayUrl,
    /// Which physical connection this is. Advances on every reconnect.
    pub connection: RelayConnection,
}

/// Transport-owned identity of one physical connection to a relay.
///
/// A transport implementation mints these values. Callers can inspect them,
/// but [`crate::OpenRelaySession`] names no connection and therefore
/// cannot select the identity a live session will wear.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelayConnection(u64);

impl RelayConnection {
    /// Construct a non-zero connection identity inside a transport implementation.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    /// Raw value for diagnostics and provider storage.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Return the next connection, or `None` instead of reusing the maximum.
    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Caller-supplied correlation for one exact frame handoff.
///
/// Authority: ARCH:1572-1576 (`send(&self, frame, correlation: HandoffCorrelation)`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HandoffCorrelation(u64);

impl HandoffCorrelation {
    /// Mint a caller-owned correlation token.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Raw caller-owned value for diagnostics.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One exact live connection to a relay, shared by every current lease holder.
///
/// Authority: ARCH:1569-1581.
pub trait RelaySession: Send + Sync {
    /// Current identity. The connection changes under the holder on reconnect.
    fn identity(&self) -> RelaySessionIdentity;

    /// Hand one complete, already-encoded frame to the relay.
    ///
    /// This is the implementer's obligation, not a caller's tool: every way to
    /// reach a relay is a verb below, and the verbs own the envelope so that
    /// NIP-01 is written in exactly one place. Nothing outside a `Transport`
    /// implementation calls this.
    ///
    /// MUST NOT park indefinitely: the outbound queue is bounded and
    /// `deadlines.write` applies. A full queue is `NotHandedOff`, never a wait.
    fn hand_off(&self, frame: Vec<u8>, correlation: HandoffCorrelation) -> HandoffFuture<'_>;

    /// Mint the next wire subscription identifier for this session.
    ///
    /// Identifiers are unique among the subscriptions live on one session by
    /// construction. Implementations produce them with [`subscription_id`].
    fn mint_subscription_id(&self) -> SubscriptionId;

    /// This session's router: which handle owns each live wire key.
    ///
    /// The implementation owns the socket and feeds decoded messages in; the
    /// router decides where each one goes, so every transport shares one
    /// answer rather than writing its own.
    fn router(&self) -> &crate::Router;

    /// Bounded queue depth for one handle, from the caller's declared bounds.
    fn inbound_capacity(&self) -> usize;

    /// Enqueue one already-encoded frame without awaiting its outcome.
    ///
    /// This is the non-awaiting half of [`RelaySession::hand_off`], and exists
    /// because releasing a subscription handle must still tell the relay, and
    /// `Drop` cannot await.
    fn enqueue(&self, frame: Vec<u8>);

    /// Open one wire subscription carrying `filters`.
    ///
    /// The session names the subscription; the caller supplies only what to
    /// match. The returned identifier is the one the frame carried.
    fn req(&self, filters: Vec<Filter>) -> ReqFuture<'_> {
        Box::pin(async move {
            let id = self.mint_subscription_id();
            let message = ClientMessage::Req {
                subscription_id: std::borrow::Cow::Owned(id.clone()),
                filters: filters.into_iter().map(std::borrow::Cow::Owned).collect(),
            };
            let handoff = self.encoded(&message).await;
            OpenedSubscription { id, handoff }
        })
    }

    /// Close one wire subscription this session opened.
    fn close_subscription(&self, id: SubscriptionId) -> HandoffFuture<'_> {
        Box::pin(async move { self.encoded(&ClientMessage::close(id)).await })
    }

    /// Publish one signed event.
    fn event(&self, event: Event) -> HandoffFuture<'_> {
        Box::pin(async move { self.encoded(&ClientMessage::event(event)).await })
    }

    /// Answer one NIP-42 challenge with a signed authentication event.
    fn auth(&self, event: Event) -> HandoffFuture<'_> {
        Box::pin(async move { self.encoded(&ClientMessage::auth(event)).await })
    }

    /// Encode one client message and hand it off. Implementers do not override
    /// this: it is the single place a Fava client message becomes bytes.
    #[doc(hidden)]
    fn encoded<'a>(&'a self, message: &'a ClientMessage<'a>) -> HandoffFuture<'a> {
        Box::pin(async move {
            match encode_client(message) {
                Ok(frame) => {
                    self.hand_off(frame.into_bytes(), HandoffCorrelation::new(0))
                        .await
                }
                Err(error) => HandoffOutcome::NotHandedOff {
                    identity: self.identity(),
                    correlation: HandoffCorrelation::new(0),
                    reason: TransportFailure::Disconnected {
                        detail: crate::BoundedText::new(format!("REQ encoding failed: {error}")),
                    },
                },
            }
        })
    }

    /// Close this session's current connection deterministically, regardless of
    /// remaining leases. Callers hold leases; this is the transport's own
    /// escape hatch and is idempotent.
    fn close(&self) -> ReleaseFuture<'_>;
}

#[cfg(test)]
mod tests {
    use super::RelayConnection;

    #[test]
    fn the_maximum_connection_has_no_successor() {
        let maximum = RelayConnection::new(u64::MAX).expect("non-zero");
        assert_eq!(maximum.checked_next(), None);
        assert_eq!(maximum.get(), u64::MAX);
    }
}

/// One relay connection, and everything true of it right now.
///
/// The two states are independent questions. Connectivity is whether a socket
/// exists; authentication is how far NIP-42 has got on the socket that does.
/// A replacement connection carries a new `identity` and begins at
/// [`Authentication::None`], because nothing proved to the relay outlives the
/// connection that proved it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Connection {
    /// Which relay, and which physical connection to it.
    pub identity: RelaySessionIdentity,
    /// Whether a socket exists.
    pub connectivity: fava_relay::Connectivity,
    /// How far authentication has got on it.
    pub authentication: fava_relay::Authentication,
}

impl Connection {
    /// A connection that is being opened and has offered nothing.
    #[must_use]
    pub const fn opening(identity: RelaySessionIdentity) -> Self {
        Self {
            identity,
            connectivity: fava_relay::Connectivity::Connecting,
            authentication: fava_relay::Authentication::None,
        }
    }

    /// Whether this connection can carry work needing `authority`.
    ///
    /// A connection with no socket carries nothing, whatever it once proved.
    #[must_use]
    pub fn can_serve(&self, authority: &fava_relay::Authority) -> bool {
        matches!(self.connectivity, fava_relay::Connectivity::Connected)
            && self.authentication.can_serve(authority)
    }
}

/// Publish the session `weak` names on `requests` each time its relay asks it
/// to authenticate.
///
/// A transport implementation spawns this once per session it opens. It is
/// here rather than in each implementation because the rule is the contract's:
/// one event per request, a repeated identical challenge is not a new request,
/// and a replaced connection asks again from nothing.
///
/// Takes only a weak reference, and upgrades it fresh each iteration, so a
/// session with no lease holders is still dropped and this ends with it. A
/// caller that instead keeps a strong `Arc` alive across the whole task —
/// even one this function itself would only ever downgrade — recreates
/// exactly the leak this signature exists to prevent: the task would keep the
/// session alive, and the session would need the task to end.
pub async fn publish_authentication_requests(
    weak: std::sync::Weak<dyn RelaySession>,
    requests: tokio::sync::broadcast::Sender<std::sync::Arc<dyn RelaySession>>,
) {
    let Some(initial) = weak.upgrade() else {
        return;
    };
    let mut connection = crate::RelaySessionExt::connection(&initial);
    drop(initial);
    let mut asked: Option<String> = None;
    let mut seen = connection.borrow_and_update().identity.clone();
    loop {
        let Some(session) = weak.upgrade() else {
            return;
        };
        let current = connection.borrow_and_update().clone();
        if current.identity != seen {
            seen = current.identity.clone();
            asked = None;
        }
        if let fava_relay::Authentication::Requested { challenge } = &current.authentication
            && asked.as_ref() != Some(challenge)
        {
            asked = Some(challenge.clone());
            let _ = requests.send(session);
        }
        if connection.changed().await.is_err() {
            return;
        }
    }
}
