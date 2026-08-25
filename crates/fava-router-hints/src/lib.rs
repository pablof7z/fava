//! Relay routing from Nostr reference hints and admitted event evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use fava_query::EventRecord;
use fava_relay::RelaySessionKey;
use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, RouteTarget,
    Router, RouterError, RouterSession,
};
use fava_write::EventId;
use nostr::types::RelayUrl;
use tokio::sync::watch;

/// Router contributing relays justified by Nostr references and observations.
pub struct HintRouter {
    name: String,
    evidence: Arc<Mutex<BTreeMap<EventId, BTreeSet<RelaySessionKey>>>>,
}

impl HintRouter {
    /// Construct one named hint policy.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            evidence: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Remember actual relay evidence for one admitted event record.
    pub fn remember(&self, record: &EventRecord) {
        self.evidence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(record.id())
            .or_default()
            .extend(
                record
                    .relay_occurrences()
                    .occurrences()
                    .map(|occurrence| occurrence.session.clone()),
            );
    }

    fn contribution(&self, request: &RouteRequest) -> RouteContribution {
        let mut by_target = BTreeMap::<RouteTarget, BTreeSet<RelaySessionKey>>::new();
        if let Some(event) = request.event() {
            for tag in event.tags() {
                let values = tag.as_slice();
                if values.first().map(String::as_str) != Some("e") {
                    continue;
                }
                let Some(Ok(event_id)) = values.get(1).map(|value| EventId::parse(value)) else {
                    continue;
                };
                let Some(Ok(relay)) = values
                    .get(2)
                    .filter(|value| !value.is_empty())
                    .map(|value| RelayUrl::parse(value))
                else {
                    continue;
                };
                by_target
                    .entry(RouteTarget::ReferencedEvent(event_id))
                    .or_default()
                    .insert(RelaySessionKey {
                        relay,
                        access: request.access(),
                    });
            }
        }
        let evidence = self
            .evidence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for target in request.targets() {
            let RouteTarget::ReferencedEvent(event_id) = target else {
                continue;
            };
            if let Some(known) = evidence.get(&event_id) {
                by_target
                    .entry(RouteTarget::ReferencedEvent(event_id))
                    .or_default()
                    .extend(known.iter().cloned());
            }
        }
        drop(evidence);

        let mut destinations = Vec::new();
        let mut coverage = BTreeMap::new();
        for target in request
            .targets()
            .into_iter()
            .filter(|target| matches!(target, RouteTarget::ReferencedEvent(_)))
        {
            let sessions = by_target.remove(&target).unwrap_or_default();
            if sessions.is_empty() {
                coverage.insert(target, CoverageState::SettledAbsent);
            } else {
                coverage.insert(target.clone(), CoverageState::Covered(sessions.clone()));
                destinations.extend(sessions.into_iter().map(|session| {
                    RouteDestination::new(
                        session,
                        BTreeSet::from([target.clone()]),
                        "Nostr reference hint or admitted relay evidence",
                    )
                }));
            }
        }
        RouteContribution {
            destinations,
            coverage,
            unresolved: BTreeSet::new(),
            shortfalls: Vec::new(),
        }
    }
}

impl Router for HintRouter {
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
        Ok(Box::new(HintSession {
            current: self.contribution(&request),
        }))
    }
}

struct HintSession {
    current: RouteContribution,
}

impl RouterSession for HintSession {
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
