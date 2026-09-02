//! Reactive fallback relays as one independent routing policy.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::Arc;

use fava_query::{Query, QuerySnapshot};
use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, RouteTarget,
    Router, RouterError, RouterSession,
};
use nostr::types::RelayUrl;

/// Router contributing configured relays while upstream target coverage is low.
pub struct FallbackRelayRouter {
    name: String,
    relays: BTreeSet<RelayUrl>,
    minimum: NonZeroUsize,
    reads: bool,
    writes: bool,
}

impl FallbackRelayRouter {
    /// Configure one named whole-target fallback policy.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        relays: impl IntoIterator<Item = RelayUrl>,
        minimum: NonZeroUsize,
    ) -> Self {
        Self {
            name: name.into(),
            relays: relays.into_iter().collect(),
            minimum,
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

    fn contribution(&self, request: &RouteRequest, upstream: &RoutePlan) -> RouteContribution {
        if (request.is_read() && !self.reads) || (request.is_write() && !self.writes) {
            return RouteContribution::default();
        }
        let targets: BTreeSet<_> = request
            .targets()
            .into_iter()
            .filter(|target| covered(upstream, target) < self.minimum.get())
            .collect();
        if targets.is_empty() {
            return RouteContribution::default();
        }
        let sessions: BTreeSet<_> = self.relays.clone();
        let coverage = targets
            .iter()
            .cloned()
            .map(|target| {
                let state = if sessions.is_empty() {
                    CoverageState::SettledAbsent
                } else {
                    CoverageState::Covered(sessions.clone())
                };
                (target, state)
            })
            .collect::<BTreeMap<_, _>>();
        RouteContribution {
            destinations: sessions
                .into_iter()
                .map(|session| {
                    RouteDestination::new(
                        session,
                        targets.clone(),
                        format!("upstream coverage below {}", self.minimum),
                    )
                })
                .collect(),
            coverage,
            unresolved: BTreeSet::new(),
            shortfalls: Vec::new(),
        }
    }
}

impl Router for FallbackRelayRouter {
    fn name(&self) -> &str {
        &self.name
    }

    fn queries(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<Vec<Query>, RouterError> {
        Ok(Vec::new())
    }

    fn preview(
        &self,
        request: &RouteRequest,
        upstream: &RoutePlan,
        inputs: &[QuerySnapshot],
    ) -> Result<RouteContribution, RouterError> {
        if !inputs.is_empty() {
            return Err(RouterError::Refused(
                "fallback router accepts no query inputs".to_owned(),
            ));
        }
        Ok(self.contribution(request, upstream))
    }

    fn open(
        &self,
        request: RouteRequest,
        upstream: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        if !inputs.is_empty() {
            return Err(RouterError::Refused(
                "fallback router accepts no query inputs".to_owned(),
            ));
        }
        let current = self.contribution(&request, &upstream);
        Ok(Box::new(FallbackSession {
            request,
            upstream,
            current,
            relays: self.relays.clone(),
            minimum: self.minimum,
            reads: self.reads,
            writes: self.writes,
        }))
    }
}

struct FallbackSession {
    request: RouteRequest,
    upstream: Arc<RoutePlan>,
    current: RouteContribution,
    relays: BTreeSet<RelayUrl>,
    minimum: NonZeroUsize,
    reads: bool,
    writes: bool,
}

impl RouterSession for FallbackSession {
    fn current(&self) -> RouteContribution {
        self.current.clone()
    }

    fn replace(
        &mut self,
        upstream: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<RouteContribution, RouterError> {
        if !inputs.is_empty() {
            return Err(RouterError::Refused(
                "fallback router accepts no query inputs".to_owned(),
            ));
        }
        self.upstream = upstream;
        let router = FallbackRelayRouter {
            name: String::new(),
            relays: self.relays.clone(),
            minimum: self.minimum,
            reads: self.reads,
            writes: self.writes,
        };
        self.current = router.contribution(&self.request, &self.upstream);
        Ok(self.current.clone())
    }

    fn close(&mut self) {}
}

fn covered(plan: &RoutePlan, target: &RouteTarget) -> usize {
    match plan.coverage.get(target) {
        Some(CoverageState::Covered(relays)) => relays.len(),
        Some(CoverageState::Unresolved | CoverageState::SettledAbsent) | None => 0,
    }
}
