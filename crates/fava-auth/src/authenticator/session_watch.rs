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

/// How often a watch checks whether it is the last thing holding its session.
///
/// Nothing signals that the last observation on a relay ended -- a lease is
/// released, not announced -- so the watch looks. The cost is one wake per
/// watched relay per interval, and the alternative is a socket held open for
/// the life of the process by a watch with nothing left to serve.
const LAST_HOLDER_CHECK: Duration = Duration::from_millis(250);

/// How many consecutive lone checks end a watch.
///
/// One is not enough: a query that named this relay may not have finished
/// acquiring its own lease yet, and a single sample can land in that window.
const LONE_CHECKS_BEFORE_RELEASE: u32 = 4;

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
        // A watch asked for directly is a standing watch: it holds its session
        // until the session itself ends.
        self.watch_session_inner(key, false).await
    }

    /// Begin watching one session without waiting for it to be acquired.
    ///
    /// Opening a query must return the local view immediately, without waiting
    /// on any relay establishment (`.planning/REQUIREMENTS.md` LOCAL-08). A
    /// challenge arrives on the relay's own schedule, so the watch is started
    /// and not awaited.
    pub fn watch_session_soon(&self, key: RelaySessionKey) {
        if Self::account(&key).is_none() {
            return;
        }
        // Two queries on one relay need one watch between them. Claiming the
        // key here, under the same lock the watch releases it with, is what
        // makes concurrent opens settle on a single watcher.
        if !self.lock().watching.insert(key.clone()) {
            return;
        }
        let owner = self.clone();
        let token = self.cancellation();
        let _ = self
            .inner()
            .runtime
            .spawn_cancellable(WATCH_TASK, token, async move {
                // A watch that cannot start is not a reason to fail a query:
                // the observation still opens, and its evidence reports the
                // relay demanding authentication.
                // Started because a query named this relay, so it lets go when
                // that query is gone.
                // A watch that never began releases its claim, so the next
                // query on this relay can try again rather than inherit a
                // watcher that does not exist.
                if owner.watch_session_inner(key.clone(), true).await.is_err() {
                    owner.lock().watching.remove(&key);
                }
            });
    }

    async fn watch_session_inner(
        &self,
        key: RelaySessionKey,
        release_when_alone: bool,
    ) -> Result<(), WatchError> {
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
        let mut connection = fava_transport::RelaySessionExt::connection(&session);

        {
            let mut guard = self.lock();
            guard.entry(&key).reconnected(identity.connection);
        }

        let owner = self.clone();
        let token = self.cancellation();
        let transport = Arc::clone(&self.inner().transport);
        let watched = key.clone();
        let spawned = self
            .inner()
            .runtime
            .spawn_cancellable(WATCH_TASK, token, async move {
                // This owner's own lease keeps the session alive, so watching
                // for challenges would hold a socket open forever. A watch
                // exists because a query named this relay; being the only
                // holder means that query is gone, and there is nothing left to
                // serve.
                //
                // Counting consecutive lone checks rather than reacting to one
                // is what makes this reliable in both directions: a query that
                // has not finished acquiring yet leaves the watch briefly
                // alone, and one sample can land in that window.
                let mut alone_for = 0_u32;
                let mut seen = connection.borrow_and_update().identity.clone();
                loop {
                    let asked = challenges.notified();
                    if release_when_alone {
                        match transport.holders(&watched).map(NonZeroUsize::get) {
                            Some(holders) if holders > 1 => alone_for = 0,
                            Some(_) => {
                                alone_for += 1;
                                if alone_for >= LONE_CHECKS_BEFORE_RELEASE {
                                    break;
                                }
                            }
                            // The session is gone; there is nothing to release.
                            None => break,
                        }
                    }
                    while let Some(text) = challenges.take() {
                        owner.challenged(&session, &text).await;
                    }
                    {
                        let current = connection.borrow_and_update().identity.clone();
                        if current != seen {
                            owner.connection_reset(&current);
                            seen = current;
                        }
                    }
                    if challenges.is_closed() {
                        break;
                    }
                    tokio::select! {
                        () = asked => {}
                        changed = connection.changed() => {
                            if changed.is_err() {
                                break;
                            }
                        }
                        () = tokio::time::sleep(LAST_HOLDER_CHECK) => {}
                    }
                }
                let _ = lease.release().await;
                if release_when_alone {
                    owner.lock().watching.remove(&watched);
                }
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
