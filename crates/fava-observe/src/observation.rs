//! The application handle of one installed observation.

use std::sync::Arc;

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
            delivered_revision: QueryRevision::new(1),
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
            self.delivered_revision.get(),
            latest.revision.get(),
        );
        self.delivered_revision = latest.revision;
        Ok(latest)
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
