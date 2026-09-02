//! Relay routing from Nostr reference hints and admitted event evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use fava_query::{Query, QuerySnapshot};
use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, RouteTarget,
    Router, RouterError, RouterSession,
};
use fava_write::EventId;
use nostr::types::RelayUrl;

/// Router contributing relays justified by Nostr references and observations.
pub struct HintRouter {
    name: String,
}

impl HintRouter {
    /// Construct one named hint policy.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Router for HintRouter {
    fn name(&self) -> &str {
        &self.name
    }

    fn queries(
        &self,
        request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<Vec<Query>, RouterError> {
        let referenced = referenced_events(request);
        if referenced.is_empty() {
            return Ok(Vec::new());
        }
        Query::events()
            .ids(referenced)
            .map_err(|error| RouterError::Refused(error.to_string()))
            .map(|query| vec![query.cache_only()])
    }

    fn preview(
        &self,
        request: &RouteRequest,
        _upstream: &RoutePlan,
        inputs: &[QuerySnapshot],
    ) -> Result<RouteContribution, RouterError> {
        project(request, inputs)
    }

    fn open(
        &self,
        request: RouteRequest,
        _upstream: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        let current = project(&request, &inputs)?;
        Ok(Box::new(HintSession { request, current }))
    }
}

struct HintSession {
    request: RouteRequest,
    current: RouteContribution,
}

impl RouterSession for HintSession {
    fn current(&self) -> RouteContribution {
        self.current.clone()
    }

    fn replace(
        &mut self,
        _upstream: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<RouteContribution, RouterError> {
        let next = project(&self.request, &inputs)?;
        self.current = next.clone();
        Ok(next)
    }

    fn close(&mut self) {}
}

fn referenced_events(request: &RouteRequest) -> BTreeSet<EventId> {
    request
        .targets()
        .into_iter()
        .filter_map(|target| match target {
            RouteTarget::ReferencedEvent(event_id) => Some(event_id),
            _ => None,
        })
        .collect()
}

fn project(
    request: &RouteRequest,
    inputs: &[QuerySnapshot],
) -> Result<RouteContribution, RouterError> {
    if inputs.len() > 1 {
        return Err(RouterError::Refused(
            "hint router declares at most one query".to_owned(),
        ));
    }
    let mut by_target = BTreeMap::<RouteTarget, BTreeSet<RelayUrl>>::new();
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
                .insert(relay);
        }
    }
    for record in inputs
        .first()
        .into_iter()
        .flat_map(|snapshot| snapshot.events.iter())
    {
        by_target
            .entry(RouteTarget::ReferencedEvent(record.id()))
            .or_default()
            .extend(
                record
                    .relay_occurrences()
                    .occurrences()
                    .map(|occurrence| occurrence.session.clone()),
            );
    }

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
    Ok(RouteContribution {
        destinations,
        coverage,
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
    })
}
