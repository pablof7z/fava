//! A fake relay session and the adversarial controls a test drives it with.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use fava_transport::{
    BoundedText, HandoffCorrelation, HandoffFuture, HandoffOutcome, OpenRelaySession,
    RelayConnection, RelaySession, RelaySessionIdentity, ReleaseFuture, ReleaseOutcome,
    TransportAmbiguity, TransportBounds, TransportDeadlines, TransportFailure,
};

use crate::stream::LiveIdentity;

pub(crate) struct FakeSession {
    pub(crate) identity: Arc<LiveIdentity>,
    pub(crate) bounds: TransportBounds,
    pub(crate) deadlines: TransportDeadlines,
    pub(crate) reconnect_attempts: usize,
    pub(crate) dials: Arc<AtomicUsize>,
    pub(crate) generations: Arc<AtomicU64>,
    /// Monotonic source of this session's wire subscription identifiers.
    pub(crate) subscriptions: Arc<AtomicU64>,
    pub(crate) router: fava_transport::Router,
    pub(crate) inner: Mutex<SessionState>,
}

#[derive(Default)]
pub(crate) struct SessionState {
    pub(crate) closed: bool,
    pub(crate) queued: Vec<Vec<u8>>,
    pub(crate) delivered: Vec<Vec<u8>>,
    pub(crate) in_flight: Vec<HandoffCorrelation>,
    pub(crate) writer_stalled: bool,
    pub(crate) queue_blocked: bool,
    pub(crate) refuse_reconnect: Option<String>,
    pub(crate) completions: Vec<HandoffOutcome>,
    pub(crate) cancelled: Vec<HandoffCorrelation>,
}

impl FakeSession {
    pub(crate) fn new(
        request: &OpenRelaySession,
        dials: Arc<AtomicUsize>,
        generation: RelayConnection,
        generations: Arc<AtomicU64>,
        subscriptions: Arc<AtomicU64>,
    ) -> Self {
        dials.fetch_add(1, Ordering::SeqCst);
        Self {
            identity: Arc::new(LiveIdentity {
                key: request.key.clone(),
                generation: AtomicU64::new(generation.get()),
            }),
            bounds: request.bounds,
            deadlines: request.deadlines,
            reconnect_attempts: request.reconnect_attempts.map_or(0, std::num::NonZero::get),
            dials,
            generations,
            subscriptions,
            router: fava_transport::Router::new(fava_transport::Connection {
                connectivity: fava_transport::Connectivity::Connected,
                ..fava_transport::Connection::opening(RelaySessionIdentity {
                    key: request.key.clone(),
                    connection: generation,
                })
            }),
            inner: Mutex::new(SessionState::default()),
        }
    }

    pub(crate) fn state(&self) -> std::sync::MutexGuard<'_, SessionState> {
        self.inner.lock().expect("fake session is not poisoned")
    }

    pub(crate) fn mark_closed(&self) {
        self.router.close();
        let mut state = self.state();
        state.closed = true;
    }

    /// Convert every frame still inside the socket into an ambiguous
    /// completion: the transport cannot prove the relay did not receive it.
    fn strand_in_flight(state: &mut SessionState, identity: &RelaySessionIdentity, detail: &str) {
        let stranded: Vec<_> = state.in_flight.drain(..).collect();
        for correlation in stranded {
            state.completions.push(HandoffOutcome::Ambiguous {
                identity: identity.clone(),
                correlation,
                reason: TransportAmbiguity::DisconnectedInFlight {
                    detail: BoundedText::new(detail),
                },
            });
        }
        state.queued.clear();
    }

    pub(crate) fn fail(&self, detail: &str) {
        let identity = self.identity.read();
        let mut state = self.state();
        Self::strand_in_flight(&mut state, &identity, detail);
        let refusal = state.refuse_reconnect.clone();
        if refusal.is_some() {
            state.closed = true;
        }
        drop(state);

        // A refused reconnect is final and says how many attempts it spent;
        // an ordinary drop may still come back and has none to report.
        let (final_detail, attempts) = refusal.as_ref().map_or((detail, None), |refusal| {
            (refusal.as_str(), Some(self.reconnect_attempts))
        });
        self.router.moved(|connection| {
            connection.connectivity = fava_transport::Connectivity::Disconnected {
                detail: BoundedText::new(final_detail),
                spent: attempts,
            };
        });
    }

    pub(crate) fn reconnect(&self) {
        let previous = self.identity.read();
        let mut state = self.state();
        Self::strand_in_flight(&mut state, &previous, "session reconnected");

        let next = self
            .generations
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_add(1)
            })
            .ok()
            .and_then(|previous| RelayConnection::new(previous + 1));
        let Some(next) = next else {
            state.closed = true;
            drop(state);
            self.router.moved(|connection| {
                connection.connectivity = fava_transport::Connectivity::Disconnected {
                    detail: BoundedText::new("generations exhausted"),
                    spent: Some(0),
                };
            });
            return;
        };
        drop(state);
        self.identity.generation.store(next.get(), Ordering::SeqCst);
        self.dials.fetch_add(1, Ordering::SeqCst);
        // A replacement has proved nothing to the relay.
        let identity = self.identity.read();
        self.router.moved(|connection| {
            *connection = fava_transport::Connection {
                identity,
                connectivity: fava_transport::Connectivity::Connected,
                authentication: fava_transport::Authentication::None,
            };
        });
    }

    pub(crate) fn push_frame(&self, frame: &[u8]) {
        // Decode once and route, exactly as the real driver does: a fake that
        // only fed the legacy path would let a consumer pass here and fail
        // against a relay.
        match std::str::from_utf8(frame)
            .ok()
            .and_then(|text| fava_wire::decode_relay(text).ok())
        {
            Some(message) => self.router.deliver(message),
            None => self.router.undecodable(),
        }
    }
}

/// Records a cancelled handoff when the caller drops the send future before it
/// resolves. Mid-operation cancellation must leave no half-frame behind.
struct CancelledHandoff<'a> {
    session: &'a FakeSession,
    correlation: HandoffCorrelation,
    armed: bool,
}

impl Drop for CancelledHandoff<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.session.state().cancelled.push(self.correlation);
        }
    }
}

impl RelaySession for FakeSession {
    fn identity(&self) -> RelaySessionIdentity {
        self.identity.read()
    }

    fn router(&self) -> &fava_transport::Router {
        &self.router
    }

    fn inbound_capacity(&self) -> usize {
        self.bounds.inbound_frames.get()
    }

    fn enqueue(&self, frame: Vec<u8>) {
        let mut state = self.state();
        if state.closed {
            return;
        }
        state.delivered.push(frame);
    }

    fn mint_subscription_id(&self) -> fava_transport::SubscriptionId {
        fava_transport::subscription_id(self.subscriptions.fetch_add(1, Ordering::SeqCst))
    }

    fn hand_off(&self, frame: Vec<u8>, correlation: HandoffCorrelation) -> HandoffFuture<'_> {
        Box::pin(async move {
            let identity = self.identity.read();
            let refuse = |reason| HandoffOutcome::NotHandedOff {
                identity: identity.clone(),
                correlation,
                reason,
            };
            if frame.len() > self.bounds.max_frame_bytes.get() {
                return refuse(TransportFailure::FrameTooLarge {
                    bytes: frame.len(),
                    maximum: self.bounds.max_frame_bytes.get(),
                });
            }

            let mut guard = CancelledHandoff {
                session: self,
                correlation,
                armed: true,
            };
            let blocked = {
                let state = self.state();
                if state.closed {
                    guard.armed = false;
                    return refuse(TransportFailure::SessionClosed);
                }
                state.queue_blocked
            };
            if blocked {
                // A blocked queue is a bounded wait for capacity, never an
                // unbounded park: `deadlines.write` ends it either way.
                tokio::time::sleep(self.deadlines.write).await;
                guard.armed = false;
                return refuse(TransportFailure::OutboundQueueFull {
                    capacity: self.bounds.outbound_frames.get(),
                });
            }

            let mut state = self.state();
            if state.queued.len() >= self.bounds.outbound_frames.get() {
                guard.armed = false;
                return refuse(TransportFailure::OutboundQueueFull {
                    capacity: self.bounds.outbound_frames.get(),
                });
            }
            if state.writer_stalled {
                state.queued.push(frame);
                state.in_flight.push(correlation);
            } else {
                state.delivered.push(frame);
            }
            guard.armed = false;
            HandoffOutcome::HandedOff {
                identity,
                correlation,
            }
        })
    }

    fn close(&self) -> ReleaseFuture<'_> {
        Box::pin(async move {
            self.mark_closed();
            Ok(ReleaseOutcome::Closed)
        })
    }
}
