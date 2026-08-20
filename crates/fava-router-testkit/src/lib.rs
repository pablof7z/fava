//! Deterministic controllable router for routing conformance and facade tests.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use fava_routing::{
    RouteContribution, RoutePlan, RouteRequest, Router, RouterError, RouterSession,
};
use tokio::sync::watch;

/// Router whose complete contribution is replaced explicitly by a test.
pub struct DelayedRouter {
    name: String,
    current: watch::Sender<Arc<RouteContribution>>,
    opens: AtomicU64,
}

impl DelayedRouter {
    /// Construct one controllable router with an immediate initial contribution.
    #[must_use]
    pub fn new(name: impl Into<String>, initial: RouteContribution) -> Self {
        let (current, _) = watch::channel(Arc::new(initial));
        Self {
            name: name.into(),
            current,
            opens: AtomicU64::new(0),
        }
    }

    /// Replace the router's complete current contribution.
    pub fn replace(&self, contribution: RouteContribution) {
        self.current.send_replace(Arc::new(contribution));
    }

    /// Number of live opens requested from this router.
    #[must_use]
    pub fn open_count(&self) -> u64 {
        self.opens.load(Ordering::SeqCst)
    }
}

impl Router for DelayedRouter {
    fn name(&self) -> &str {
        &self.name
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        Ok(self.current.borrow().as_ref().clone())
    }

    fn open(
        &self,
        _request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(DelayedSession {
            current: self.current.subscribe(),
        }))
    }
}

struct DelayedSession {
    current: watch::Receiver<Arc<RouteContribution>>,
}

impl RouterSession for DelayedSession {
    fn current(&self) -> RouteContribution {
        self.current.borrow().as_ref().clone()
    }

    fn next_change(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<RouteContribution, RouterError>> + Send + '_>> {
        Box::pin(async move {
            self.current
                .changed()
                .await
                .map_err(|_| RouterError::Closed)?;
            Ok(self.current.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {}
}
