//! The application handle of one installed observation.

use std::sync::Arc;
use std::time::Duration;

use fava_query::{ObservationId, QueryRevision, QuerySnapshot};
use tokio::sync::watch;

use crate::error::ObservationClosed;
use crate::registry::Registry;
use crate::sources::{Coalesced, report_delivery_gap};

/// One opened latest-state live query.
pub struct Observation {
    id: ObservationId,
    registry: Arc<Registry>,
    latest: watch::Receiver<Arc<QuerySnapshot>>,
    cancelled: fava_runtime::CancellationToken,
    delivered_revision: QueryRevision,
    coalesced: Option<Coalesced>,
}

impl Observation {
    pub(crate) fn new(
        id: ObservationId,
        registry: Arc<Registry>,
        latest: watch::Receiver<Arc<QuerySnapshot>>,
        cancelled: fava_runtime::CancellationToken,
        coalesced: Option<Coalesced>,
    ) -> Self {
        Self {
            id,
            registry,
            latest,
            cancelled,
            delivered_revision: QueryRevision(1),
            coalesced,
        }
    }

    /// Exact installed-owner identity of this observation.
    #[must_use]
    pub const fn id(&self) -> ObservationId {
        self.id
    }

    /// Exact current snapshot, readable immediately after open.
    #[must_use]
    pub fn current(&self) -> Arc<QuerySnapshot> {
        Arc::clone(&self.latest.borrow())
    }

    /// Await a newer delivered current state.
    ///
    /// # Errors
    ///
    /// Returns [`ObservationClosed`] after explicit close, provider
    /// termination, evaluation failure, or engine teardown.
    pub async fn changed(&mut self) -> Result<Arc<QuerySnapshot>, ObservationClosed> {
        if self.cancelled.is_cancelled() {
            return Err(ObservationClosed);
        }
        self.latest.changed().await.map_err(|_| ObservationClosed)?;
        if self.cancelled.is_cancelled() {
            return Err(ObservationClosed);
        }
        let latest = Arc::clone(&self.latest.borrow_and_update());
        report_delivery_gap(
            self.coalesced.as_deref(),
            self.delivered_revision.0,
            latest.revision.0,
        );
        self.delivered_revision = latest.revision;
        Ok(latest)
    }

    /// Wait, within one caller-supplied bound, for a current snapshot that
    /// satisfies `predicate`.
    ///
    /// The predicate first sees the snapshot available when this method is
    /// called. Later checks see only snapshots delivered by this same
    /// observation. It runs at most once for each snapshot this call observes.
    /// This method neither opens work nor changes the observation lifecycle.
    /// A timed-out wait leaves the observation open, so a later call can still
    /// receive a snapshot that arrived at the boundary.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` when the one caller-supplied bound expires without a
    /// match. Returns [`ObservationClosed`] unchanged when the observation
    /// ends. Cancelling this future leaves later delivery for the handle; a
    /// delivered snapshot advances it only through [`Self::changed`].
    pub async fn wait_until(
        &mut self,
        timeout: Duration,
        mut predicate: impl FnMut(&QuerySnapshot) -> bool,
    ) -> Result<Option<Arc<QuerySnapshot>>, ObservationClosed> {
        match tokio::time::timeout(timeout, async {
            let mut snapshot = self.current();
            loop {
                if predicate(snapshot.as_ref()) {
                    return Ok(Some(snapshot));
                }
                snapshot = self.changed().await?;
            }
        })
        .await
        {
            Ok(result) => result,
            Err(_) => Ok(None),
        }
    }

    /// Close this observation. Repeated close is harmless.
    ///
    /// Closing releases exactly this observation's demand. Wire subscriptions
    /// another observation still needs, and the relay session behind them,
    /// are untouched.
    pub fn close(&self) {
        self.registry.withdraw(self.id);
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        self.close();
    }
}
