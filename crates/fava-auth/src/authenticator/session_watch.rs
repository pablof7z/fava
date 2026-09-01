//! Answering the relays that ask, on connections somebody else opened.

use std::sync::Arc;

use fava_relay::{Authentication, BoundedText};
use fava_transport::{RelaySession, Transport};

use super::{Authenticator, WATCH_TASK};
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
    pub fn answer_requests(&self, transport: &dyn Transport) -> Result<(), WatchError> {
        let mut asked = transport.authentication_requests();
        let owner = self.clone();
        let token = self.cancellation();
        self.inner()
            .runtime
            .spawn_cancellable(WATCH_TASK, token, async move {
                loop {
                    match asked.recv().await {
                        Ok(session) => owner.asked(&session).await,
                        // Lagging means a relay asked while this owner was
                        // busy answering another. The demand is still on that
                        // connection; the next thing it does republishes it.
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
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
        let Authentication::Requested { challenge } = &connection.authentication else {
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
                    Authentication::Failed {
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
