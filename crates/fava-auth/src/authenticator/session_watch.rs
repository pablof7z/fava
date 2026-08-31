//! Watching one authenticated session for challenges and verdicts.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use fava_relay::{AuthenticationState, BoundedText, RelaySessionKey};
use fava_transport::{
    OpenRelaySession, RelayInbound, RelaySession, TransportBounds, TransportDeadlines,
};
use fava_wire::{RelayMessage, decode_relay};

use thiserror::Error;

use super::{Authenticator, WATCH_TASK};
use crate::challenge::Challenge;

const MAX_INBOUND_FRAMES: usize = 64;
const SESSION_DEADLINE: Duration = Duration::from_secs(30);

/// Bounds and deadlines this owner hands the transport for its own lease.
pub(super) fn open_request(key: &RelaySessionKey) -> OpenRelaySession {
    let frames = |count: usize| NonZeroUsize::new(count).expect("constant is non-zero");
    OpenRelaySession {
        key: key.clone(),
        deadlines: TransportDeadlines {
            establish: SESSION_DEADLINE,
            write: SESSION_DEADLINE,
            idle: SESSION_DEADLINE,
            close: SESSION_DEADLINE,
        },
        bounds: TransportBounds {
            inbound_frames: frames(MAX_INBOUND_FRAMES),
            outbound_frames: frames(4),
            max_frame_bytes: frames(1_048_576),
        },
        reconnect_attempts: None,
    }
}

impl Authenticator {
    /// Begin watching one authenticated session for challenges.
    ///
    /// The owner takes its own lease, so a relay's unsolicited challenge is
    /// seen whether or not a query or publication is currently attached.
    ///
    /// # Errors
    ///
    /// Returns [`WatchError`] when the session cannot be acquired or the watch
    /// cannot be registered with the runtime.
    pub async fn watch_session(&self, key: RelaySessionKey) -> Result<(), WatchError> {
        if Self::account(&key).is_none() {
            return Ok(());
        }

        let lease = self
            .inner()
            .transport
            .acquire_session(open_request(&key))
            .await
            .map_err(WatchError::Acquire)?;
        let identity = lease.acquired_identity().clone();
        let session = Arc::clone(lease.session());
        let mut inbound = session.messages();

        {
            let mut guard = self.lock();
            guard.entry(&key).reconnected(identity.generation);
        }

        let owner = self.clone();
        let token = self.cancellation();
        let spawned = self
            .inner()
            .runtime
            .spawn_cancellable(WATCH_TASK, token, async move {
                while let Ok(item) = inbound.next_inbound().await {
                    if owner.admit(&item, &session).await.is_break() {
                        break;
                    }
                }
                let _ = lease.release().await;
            });

        spawned.map(|_| ()).map_err(WatchError::Register)
    }

    /// Handle one inbound item. Breaking ends the watch.
    async fn admit(
        &self,
        item: &RelayInbound,
        session: &Arc<dyn RelaySession>,
    ) -> std::ops::ControlFlow<()> {
        match item {
            RelayInbound::Frame {
                identity, frame, ..
            } => {
                let Ok(text) = std::str::from_utf8(frame) else {
                    return std::ops::ControlFlow::Continue(());
                };
                let Ok(message) = decode_relay(text) else {
                    return std::ops::ControlFlow::Continue(());
                };
                match message {
                    RelayMessage::Auth { challenge } => match Challenge::new(challenge.as_ref()) {
                        Ok(challenge) => {
                            self.resolve(identity.clone(), challenge, session).await;
                        }
                        Err(error) => {
                            self.record(
                                identity,
                                AuthenticationState::Failed {
                                    reason: BoundedText::new(error.to_string()),
                                },
                            );
                        }
                    },
                    RelayMessage::Ok {
                        event_id,
                        status,
                        message,
                    } => self.verdict(identity, &event_id, status, message.as_ref()),
                    _ => {}
                }
                std::ops::ControlFlow::Continue(())
            }
            RelayInbound::Reconnected { identity, .. } => {
                let signal = {
                    let mut guard = self.lock();
                    guard.entry(&identity.key).reconnected(identity.generation);
                    guard.awaiting_ok.remove(&identity.key);
                    guard.drop_deferred_before(&identity.key, identity.generation)
                };
                if signal {
                    self.signal();
                }
                std::ops::ControlFlow::Continue(())
            }
            RelayInbound::ReconnectExhausted { .. } => std::ops::ControlFlow::Break(()),
            RelayInbound::Disconnected { .. } | RelayInbound::Lost { .. } => {
                std::ops::ControlFlow::Continue(())
            }
        }
    }

    /// Classify the relay's `OK` for a challenge response we sent.
    fn verdict(
        &self,
        identity: &fava_transport::RelaySessionIdentity,
        event_id: &fava_write::EventId,
        status: bool,
        message: &str,
    ) {
        let ours = {
            let guard = self.lock();
            guard.awaiting_ok.get(&identity.key) == Some(event_id)
        };
        if !ours {
            return;
        }
        self.lock().awaiting_ok.remove(&identity.key);

        let state = if status {
            AuthenticationState::Accepted
        } else if matches!(
            nostr::message::MachineReadablePrefix::parse(message),
            Some(nostr::message::MachineReadablePrefix::Restricted)
        ) {
            AuthenticationState::AcceptedButStillRefused {
                message: BoundedText::new(message),
            }
        } else {
            AuthenticationState::Rejected {
                message: BoundedText::new(message),
            }
        };
        self.record(identity, state);
    }
}

/// Why one session's authentication watch could not begin.
#[derive(Debug, Error)]
pub enum WatchError {
    /// The session could not be acquired.
    #[error("authentication watch could not acquire the session: {0}")]
    Acquire(#[from] fava_transport::TransportError),
    /// The runtime refused to register the watch.
    #[error("authentication watch could not be registered: {0}")]
    Register(#[from] fava_runtime::RuntimeError),
}
