//! NIP-65 author-outbox and recipient-inbox routing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use fava_nip65::{RelayList, relay_lists};
use fava_query::{Query, QuerySnapshot};
use fava_relay::RelaySessionKey;
use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, RouteTarget,
    Router, RouterError, RouterSession,
};
use nostr::key::PublicKey;
use nostr::types::RelayUrl;

const MAX_AGE: Duration = Duration::from_secs(300);
const MAX_SHORTFALLS: usize = 256;

/// NIP-65 router. The observation engine owns every declared query lifecycle.
pub struct OutboxRouter {
    name: String,
    indexers: BTreeSet<RelayUrl>,
}

impl OutboxRouter {
    /// Configure one NIP-65 policy and its indexer destinations.
    pub fn new(
        name: impl Into<String>,
        indexers: impl IntoIterator<Item = RelayUrl>,
    ) -> Result<Self, RouterError> {
        Ok(Self {
            name: name.into(),
            indexers: indexers.into_iter().collect(),
        })
    }

    fn query(&self, request: &RouteRequest) -> Result<Vec<Query>, RouterError> {
        let authors = requested_authors(request);
        if authors.is_empty() || self.indexers.is_empty() {
            return Ok(Vec::new());
        }
        relay_lists(authors)
            .map_err(|error| RouterError::Refused(error.to_string()))?
            .from_relays(self.indexers.iter().cloned())
            .map_err(|error| RouterError::Refused(error.to_string()))
            .map(|query| vec![query.with_relay_access(request.access()).max_age(MAX_AGE)])
    }
}

impl Router for OutboxRouter {
    fn name(&self) -> &str {
        &self.name
    }

    fn queries(
        &self,
        request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<Vec<Query>, RouterError> {
        self.query(request)
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
        Ok(Box::new(OutboxSession {
            request,
            inputs,
            current,
            closed: false,
        }))
    }
}

struct OutboxSession {
    request: RouteRequest,
    inputs: Vec<QuerySnapshot>,
    current: RouteContribution,
    closed: bool,
}

impl RouterSession for OutboxSession {
    fn current(&self) -> RouteContribution {
        self.current.clone()
    }

    fn replace(
        &mut self,
        _upstream: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<RouteContribution, RouterError> {
        if self.closed {
            return Err(RouterError::Closed);
        }
        let next = project(&self.request, &inputs)?;
        self.inputs = inputs;
        self.current = next.clone();
        Ok(next)
    }

    fn close(&mut self) {
        self.closed = true;
    }
}

fn project(
    request: &RouteRequest,
    inputs: &[QuerySnapshot],
) -> Result<RouteContribution, RouterError> {
    if inputs.len() > 1 {
        return Err(RouterError::Refused(
            "outbox router declares at most one query".to_owned(),
        ));
    }
    let authors = requested_authors(request);
    if authors.is_empty() {
        return Ok(RouteContribution::default());
    }
    let (lists, shortfalls) = inputs.first().map(decode).unwrap_or_default();
    let settled = inputs
        .first()
        .is_some_and(|snapshot| snapshot.evidence.all_relays_stored_events_complete());
    contribution(request, &lists, settled, shortfalls)
}

fn decode(snapshot: &QuerySnapshot) -> (BTreeMap<PublicKey, RelayList>, Vec<String>) {
    let mut values: BTreeMap<PublicKey, (RelayList, nostr::types::Timestamp, fava_query::EventId)> =
        BTreeMap::new();
    let mut shortfalls = Vec::new();
    for record in snapshot.events.iter() {
        match RelayList::from_event(record.event()) {
            Ok(list) => {
                let candidate = (record.created_at(), record.id());
                let replace = values
                    .get(&list.author())
                    .is_none_or(|(_, at, id)| candidate > (*at, *id));
                if replace {
                    values.insert(list.author(), (list, candidate.0, candidate.1));
                }
            }
            Err(error) => bounded_push(&mut shortfalls, error.to_string()),
        }
    }
    (
        values
            .into_iter()
            .map(|(author, (list, _, _))| (author, list))
            .collect(),
        shortfalls,
    )
}

fn bounded_push(shortfalls: &mut Vec<String>, value: String) {
    if shortfalls.len() < MAX_SHORTFALLS {
        shortfalls.push(value);
    }
}

fn requested_authors(request: &RouteRequest) -> BTreeSet<PublicKey> {
    request
        .targets()
        .into_iter()
        .filter_map(|target| match target {
            RouteTarget::Author(author) | RouteTarget::Recipient(author) => Some(author),
            RouteTarget::WholeRequest | RouteTarget::ReferencedEvent(_) => None,
        })
        .collect()
}

fn contribution(
    request: &RouteRequest,
    lists: &BTreeMap<PublicKey, RelayList>,
    settled: bool,
    shortfalls: Vec<String>,
) -> Result<RouteContribution, RouterError> {
    let mut destinations = Vec::new();
    let mut coverage = BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    for target in request.targets() {
        let (_author, relays, reason) = match &target {
            RouteTarget::Author(author) => (
                author,
                lists.get(author).map(RelayList::write_relays),
                "NIP-65 author write relay",
            ),
            RouteTarget::Recipient(author) => (
                author,
                lists.get(author).map(RelayList::read_relays),
                "NIP-65 recipient read relay",
            ),
            RouteTarget::WholeRequest | RouteTarget::ReferencedEvent(_) => continue,
        };
        let Some(relays) = relays else {
            if settled {
                coverage.insert(target, CoverageState::SettledAbsent);
            } else {
                unresolved.insert(target.clone());
                coverage.insert(target, CoverageState::Unresolved);
            }
            continue;
        };
        let sessions: BTreeSet<_> = relays
            .iter()
            .cloned()
            .map(|relay| RelaySessionKey {
                relay,
                access: request.access(),
            })
            .collect();
        if sessions.is_empty() {
            coverage.insert(target, CoverageState::SettledAbsent);
        } else {
            coverage.insert(target.clone(), CoverageState::Covered(sessions.clone()));
            destinations.extend(sessions.into_iter().map(|session| {
                RouteDestination::new(session, BTreeSet::from([target.clone()]), reason)
            }));
        }
    }
    Ok(RouteContribution {
        destinations,
        coverage,
        unresolved,
        shortfalls,
    })
}
