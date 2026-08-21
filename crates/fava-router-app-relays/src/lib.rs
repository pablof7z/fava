//! Configured application relays as one independent routing policy.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, Router,
    RouterError, RouterSession,
};
use fava_state::RelayUrl;
use tokio::sync::watch;

/// Router that always contributes the application's configured relays.
pub struct AppRelayRouter {
    name: String,
    relays: BTreeSet<RelayUrl>,
    reads: bool,
    writes: bool,
}

impl AppRelayRouter {
    /// Configure one named app-relay policy.
    #[must_use]
    pub fn new(name: impl Into<String>, relays: impl IntoIterator<Item = RelayUrl>) -> Self {
        Self {
            name: name.into(),
            relays: relays.into_iter().collect(),
            reads: true,
            writes: true,
        }
    }

    /// Select whether this policy contributes to read routing.
    #[must_use]
    pub const fn reads(mut self, enabled: bool) -> Self {
        self.reads = enabled;
        self
    }

    /// Select whether this policy contributes to write routing.
    #[must_use]
    pub const fn writes(mut self, enabled: bool) -> Self {
        self.writes = enabled;
        self
    }

    fn contribution(&self, request: &RouteRequest) -> RouteContribution {
        if (request.is_read() && !self.reads) || (request.is_write() && !self.writes) {
            return RouteContribution::default();
        }
        let targets = request.targets();
        let sessions: BTreeSet<_> = self
            .relays
            .iter()
            .cloned()
            .map(|relay| fava_state::RelaySessionKey::new(relay, request.access()))
            .collect();
        let coverage = targets
            .iter()
            .cloned()
            .map(|target| (target, CoverageState::Covered(sessions.clone())))
            .collect::<BTreeMap<_, _>>();
        let destinations = sessions
            .into_iter()
            .map(|session| {
                RouteDestination::new(session, targets.clone(), "configured application relay")
            })
            .collect();
        RouteContribution {
            destinations,
            coverage,
            unresolved: BTreeSet::new(),
            shortfalls: Vec::new(),
        }
    }
}

impl Router for AppRelayRouter {
    fn name(&self) -> &str {
        &self.name
    }

    fn preview(
        &self,
        request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        Ok(self.contribution(request))
    }

    fn open(
        &self,
        request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        Ok(Box::new(StaticSession {
            current: self.contribution(&request),
        }))
    }
}

struct StaticSession {
    current: RouteContribution,
}

impl RouterSession for StaticSession {
    fn current(&self) -> RouteContribution {
        self.current.clone()
    }

    fn next_change(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<RouteContribution, RouterError>> + Send + '_>> {
        Box::pin(std::future::pending())
    }

    fn close(&mut self) {}
}
