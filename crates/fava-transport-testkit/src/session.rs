//! A fake relay session and the adversarial controls a test drives it with.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use fava_state::Timestamp;
use fava_transport::{
    BoundedReason, HandoffCorrelation, HandoffFuture, HandoffOutcome, OpenRelaySession,
    RelayInbound, RelayMessageStream, RelaySession, RelaySessionIdentity, ReleaseFuture,
    ReleaseOutcome, TransportAmbiguity, TransportBounds, TransportDeadlines, TransportFailure,
};

use crate::stream::{ConsumerState, FakeMessageStream, LiveIdentity};

pub(crate) struct FakeSession {
    pub(crate) identity: Arc<LiveIdentity>,
    pub(crate) bounds: TransportBounds,
    pub(crate) deadlines: TransportDeadlines,
    pub(crate) reconnect_attempts: usize,
    pub(crate) dials: Arc<AtomicUsize>,
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
    pub(crate) consumers: Vec<Arc<ConsumerState>>,
}

impl FakeSession {
    pub(crate) fn new(request: &OpenRelaySession, dials: Arc<AtomicUsize>) -> Self {
        dials.fetch_add(1, Ordering::SeqCst);
        Self {
            identity: Arc::new(LiveIdentity {
                key: request.key.clone(),
                generation: AtomicU64::new(1),
            }),
            bounds: request.bounds,
            deadlines: request.deadlines,
            reconnect_attempts: request.reconnect_attempts.map_or(0, std::num::NonZero::get),
            dials,
            inner: Mutex::new(SessionState::default()),
        }
    }

    pub(crate) fn state(&self) -> std::sync::MutexGuard<'_, SessionState> {
        self.inner.lock().expect("fake session is not poisoned")
    }

    pub(crate) fn fan_out(state: &SessionState, item: &RelayInbound) {
        for consumer in &state.consumers {
            consumer.offer(item.clone());
        }
    }

    pub(crate) fn mark_closed(&self) {
        let mut state = self.state();
        state.closed = true;
        for consumer in &state.consumers {
            consumer.detach();
        }
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
                    detail: BoundedReason::new(detail),
                },
            });
        }
        state.queued.clear();
    }

    pub(crate) fn fail(&self, detail: &str) {
        let identity = self.identity.read();
        let mut state = self.state();
        Self::strand_in_flight(&mut state, &identity, detail);
        let disconnected = RelayInbound::Disconnected {
            identity: identity.clone(),
            reason: TransportFailure::Disconnected {
                detail: BoundedReason::new(detail),
            },
        };
        Self::fan_out(&state, &disconnected);

        if let Some(refusal) = state.refuse_reconnect.clone() {
            let exhausted = RelayInbound::ReconnectExhausted {
                identity,
                attempts: self.reconnect_attempts,
                reason: TransportFailure::Disconnected {
                    detail: BoundedReason::new(refusal),
                },
            };
            Self::fan_out(&state, &exhausted);
            state.closed = true;
        }
    }

    pub(crate) fn reconnect(&self) {
        let previous = self.identity.read();
        let mut state = self.state();
        Self::strand_in_flight(&mut state, &previous, "session reconnected");
        let disconnected = RelayInbound::Disconnected {
            identity: previous.clone(),
            reason: TransportFailure::Disconnected {
                detail: BoundedReason::new("session reconnected"),
            },
        };
        Self::fan_out(&state, &disconnected);

        self.identity.generation.fetch_add(1, Ordering::SeqCst);
        self.dials.fetch_add(1, Ordering::SeqCst);
        let reconnected = RelayInbound::Reconnected {
            previous,
            identity: self.identity.read(),
        };
        Self::fan_out(&state, &reconnected);
    }

    pub(crate) fn push_frame(&self, frame: Vec<u8>) {
        let identity = self.identity.read();
        let state = self.state();
        Self::fan_out(
            &state,
            &RelayInbound::Frame {
                identity,
                frame,
                received_at: Timestamp::now(),
            },
        );
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

    fn send(&self, frame: Vec<u8>, correlation: HandoffCorrelation) -> HandoffFuture<'_> {
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

    fn messages(&self) -> Box<dyn RelayMessageStream> {
        let consumer = Arc::new(ConsumerState::new(self.bounds.inbound_frames.get()));
        self.state().consumers.push(Arc::clone(&consumer));
        Box::new(FakeMessageStream {
            consumer,
            identity: Arc::clone(&self.identity),
            idle: self.deadlines.idle,
        })
    }

    fn close(&self) -> ReleaseFuture<'_> {
        Box::pin(async move {
            self.mark_closed();
            Ok(ReleaseOutcome::Closed)
        })
    }
}
