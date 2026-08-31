//! Watching one authenticated session for challenges and verdicts.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use fava_relay::{AuthenticationState, BoundedText, RelaySessionKey};
use fava_transport::{OpenRelaySession, RelaySession, TransportBounds, TransportDeadlines};

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
        // This owner holds no subscription and publishes nothing, so it reads
        // the two facts it actually needs: the relay's challenges, and whether
        // its connection was replaced.
        let challenges = fava_transport::RelaySessionExt::challenges(&session);
        let connection = fava_transport::RelaySessionExt::connection(&session);

        {
            let mut guard = self.lock();
            guard.entry(&key).reconnected(identity.connection);
        }

        let owner = self.clone();
        let token = self.cancellation();
        let spawned = self
            .inner()
            .runtime
            .spawn_cancellable(WATCH_TASK, token, async move {
                loop {
                    let asked = challenges.notified();
                    let changed = connection.notified();
                    while let Some(text) = challenges.take() {
                        owner.challenged(&session, &text).await;
                    }
                    while let Some(state) = connection.take() {
                        if let fava_transport::ConnectionState::Reconnected { identity } = state {
                            owner.connection_reset(&identity);
                        }
                    }
                    if challenges.is_closed() && connection.is_closed() {
                        break;
                    }
                    tokio::select! {
                        () = asked => {}
                        () = changed => {}
                    }
                }
                let _ = lease.release().await;
            });

        spawned.map(|_| ()).map_err(WatchError::Register)
    }

    /// One challenge the relay sent on this session's current connection.
    async fn challenged(&self, session: &Arc<dyn RelaySession>, text: &str) {
        let identity = session.identity();
        match Challenge::new(text) {
            Ok(challenge) => {
                let _ = self.resolve(identity, challenge, session).await;
            }
            Err(error) => {
                self.record(
                    &identity,
                    AuthenticationState::Failed {
                        reason: BoundedText::new(error.to_string()),
                    },
                );
            }
        }
    }

    /// The connection was replaced. Everything proved to the relay, and every
    /// question parked awaiting a person, belonged to the connection that died.
    fn connection_reset(&self, current: &fava_transport::RelaySessionIdentity) {
        let signal = {
            let mut guard = self.lock();
            guard.entry(&current.key).reconnected(current.connection);
            guard.drop_deferred_before(&current.key, current.connection)
        };
        if signal {
            self.signal();
        }
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
