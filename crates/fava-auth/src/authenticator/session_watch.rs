//! Answering the relays that ask, on connections somebody else opened.

use std::sync::Arc;

use fava_relay::{BoundedText, Progress};
use fava_transport::{RelaySession, Transport};

use super::{ANSWER_TASK, Authenticator, WATCH_TASK};
use crate::challenge::Challenge;

impl Authenticator {
    /// Answer every relay that asks, for as long as this owner lives.
    ///
    /// The transport says which session was asked. This owner opens no
    /// connection, holds none, and knows nothing of the work waiting on one:
    /// it decides, it answers, and the connection moving is what lets that
    /// work carry on.
    ///
    /// # Errors
    ///
    /// Returns [`WatchError`] when the runtime refuses to register the task.
    pub fn answer_requests(&self, transport: &Arc<dyn Transport>) -> Result<(), WatchError> {
        let mut asked = transport.authentication_requests();
        let waiting = Arc::clone(transport);
        let owner = self.clone();
        let runtime = self.inner().runtime.clone();
        let watch_token = self.cancellation();
        let answer_token = watch_token.clone();
        self.inner()
            .runtime
            .spawn_cancellable(WATCH_TASK, watch_token, async move {
                loop {
                    match asked.recv().await {
                        Ok(session) => {
                            // Answering must not block reading the next ask:
                            // a signer can be remote and slow, and a relay
                            // that is kept waiting because a different
                            // relay's signer is slow has been answered by
                            // nothing this owner did. Spawned rather than
                            // awaited, so one slow signer costs this loop
                            // nothing.
                            let answering = owner.clone();
                            let token = answer_token.clone();
                            let _ = runtime.spawn_cancellable(ANSWER_TASK, token, async move {
                                answering.asked(&session).await;
                            });
                        }
                        // The buffer overflowed: this owner fell behind the
                        // relays it watches. What overflowed is genuinely
                        // gone — a relay is republished only when its
                        // challenge changes, and this owner never saw the one
                        // it lost. So rather than wait for a repetition that
                        // is not coming, ask which connections are still
                        // waiting to be answered and answer those.
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            owner.record_lagged(skipped);
                            for session in waiting.awaiting_authentication() {
                                let answering = owner.clone();
                                let token = answer_token.clone();
                                let _ = runtime.spawn_cancellable(ANSWER_TASK, token, async move {
                                    answering.asked(&session).await;
                                });
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                    }
                }
            })
            .map(|_| ())
            .map_err(WatchError::Register)
    }

    /// One relay asked this session to authenticate.
    async fn asked(&self, session: &Arc<dyn RelaySession>) {
        let connection = fava_transport::RelaySessionExt::connection(session)
            .borrow()
            .clone();
        let Progress::Requested { challenge } = &connection.authentication.progress else {
            // It has moved on since the transport published it. Whatever it
            // moved to, this is no longer a question.
            return;
        };
        let identity = connection.identity.clone();
        match Challenge::new(challenge) {
            Ok(challenge) => {
                let _ = self.resolve(identity, challenge, session).await;
            }
            Err(error) => {
                self.record(
                    &identity,
                    session,
                    Progress::Unanswerable {
                        reason: BoundedText::new(error.to_string()),
                    },
                );
            }
        }
    }
}

/// Why this owner could not begin answering.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    /// The runtime refused to register the task.
    #[error("authentication answering could not be registered: {0}")]
    Register(#[from] fava_runtime::RuntimeError),
}
