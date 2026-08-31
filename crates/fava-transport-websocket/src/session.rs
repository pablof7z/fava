//! One live WebSocket relay session shared by every current lease holder.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use fava_transport::{
    HandoffCorrelation, HandoffFuture, HandoffOutcome, OpenRelaySession, RelayMessageStream,
    RelaySession, RelaySessionGeneration, RelaySessionIdentity, ReleaseFuture, ReleaseOutcome,
    TransportAmbiguity, TransportBounds, TransportDeadlines, TransportFailure,
};
use tokio::sync::{Notify, mpsc, oneshot};

use crate::driver::Outbound;
use crate::fanout::{Consumers, LiveIdentity, WebSocketMessageStream};

/// State shared between a session handle and the task driving its socket.
pub(crate) struct SessionShared {
    pub(crate) identity: Arc<LiveIdentity>,
    pub(crate) bounds: TransportBounds,
    pub(crate) deadlines: TransportDeadlines,
    pub(crate) reconnect_attempts: Option<NonZeroUsize>,
    pub(crate) consumers: Consumers,
    pub(crate) closed: AtomicBool,
    pub(crate) close_requested: Notify,
    pub(crate) close_finished: Notify,
    pub(crate) entropy: u64,
    pub(crate) generations: Arc<AtomicU64>,
    /// Monotonic source of this session's wire subscription identifiers.
    pub(crate) subscriptions: Arc<AtomicU64>,
}

impl SessionShared {
    pub(crate) fn new(
        request: &OpenRelaySession,
        entropy: u64,
        generation: RelaySessionGeneration,
        generations: Arc<AtomicU64>,
        subscriptions: Arc<AtomicU64>,
    ) -> Self {
        Self {
            identity: Arc::new(LiveIdentity::new(request.key.clone(), generation)),
            bounds: request.bounds,
            deadlines: request.deadlines,
            reconnect_attempts: request.reconnect_attempts,
            consumers: Consumers::default(),
            closed: AtomicBool::new(false),
            close_requested: Notify::new(),
            close_finished: Notify::new(),
            entropy,
            generations,
            subscriptions,
        }
    }
}

/// The `RelaySession` every lease holder shares.
pub(crate) struct WebSocketRelaySession {
    pub(crate) shared: Arc<SessionShared>,
    pub(crate) outbound: mpsc::Sender<Outbound>,
}

impl RelaySession for WebSocketRelaySession {
    fn identity(&self) -> RelaySessionIdentity {
        self.shared.identity.read()
    }

    fn mint_subscription_id(&self) -> fava_transport::SubscriptionId {
        fava_transport::subscription_id(self.shared.subscriptions.fetch_add(1, Ordering::SeqCst))
    }

    fn hand_off(&self, frame: Vec<u8>, correlation: HandoffCorrelation) -> HandoffFuture<'_> {
        Box::pin(async move {
            let identity = self.shared.identity.read();
            let refuse = |reason| HandoffOutcome::NotHandedOff {
                identity: identity.clone(),
                correlation,
                reason,
            };
            let maximum = self.shared.bounds.max_frame_bytes.get();
            if frame.len() > maximum {
                return refuse(TransportFailure::FrameTooLarge {
                    bytes: frame.len(),
                    maximum,
                });
            }
            if self.shared.closed.load(Ordering::SeqCst) {
                return refuse(TransportFailure::SessionClosed);
            }

            let (completion, settled) = oneshot::channel();
            match self.outbound.try_send(Outbound {
                frame,
                correlation,
                completion,
            }) {
                Err(mpsc::error::TrySendError::Full(_)) => {
                    // A full queue refuses. It never parks the caller, so one
                    // stalled relay cannot hold an unrelated sender hostage.
                    return refuse(TransportFailure::OutboundQueueFull {
                        capacity: self.shared.bounds.outbound_frames.get(),
                    });
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    return refuse(TransportFailure::SessionClosed);
                }
                Ok(()) => {}
            }

            let deadline = self.shared.deadlines.write;
            match tokio::time::timeout(deadline, settled).await {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(_)) => refuse(TransportFailure::SessionClosed),
                Err(_) => HandoffOutcome::Ambiguous {
                    identity,
                    correlation,
                    reason: TransportAmbiguity::WriteTimeout { after: deadline },
                },
            }
        })
    }

    fn messages(&self) -> Box<dyn RelayMessageStream> {
        Box::new(WebSocketMessageStream {
            consumer: self
                .shared
                .consumers
                .register(self.shared.bounds.inbound_frames.get()),
            identity: Arc::clone(&self.shared.identity),
        })
    }

    fn close(&self) -> ReleaseFuture<'_> {
        Box::pin(async move {
            let finished = self.shared.close_finished.notified();
            if self.shared.closed.swap(true, Ordering::SeqCst) {
                self.shared.consumers.detach_all();
                return Ok(ReleaseOutcome::Closed);
            }
            // The driver owns the handshake and its deadline. Fava reports the
            // session closed once that deadline passes, whatever the peer does.
            self.shared.close_requested.notify_one();
            let _ =
                tokio::time::timeout(self.shared.deadlines.close.saturating_mul(2), finished).await;
            self.shared.consumers.detach_all();
            Ok(ReleaseOutcome::Closed)
        })
    }
}
