//! NIP-65 author-outbox and recipient-inbox routing.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use fava_nip65::{RelayList, relay_lists};
use fava_query::{
    OpenedQuerySource, QuerySource, QuerySourceClosed, SourceChanges, SourceEvent, SourceSnapshot,
    SourceStatus, SourceTerminationCause,
};
use fava_relay::RelaySessionKey;
use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, RouteTarget,
    Router, RouterError, RouterSession,
};
use fava_write::EventValue;
use nostr::key::PublicKey;
use nostr::types::RelayUrl;
use tokio::sync::watch;

const MAX_SHORTFALLS: usize = 256;

/// Bounded shortfall evidence that counts, never hides, discarded entries.
#[derive(Clone, Debug, Default)]
struct Shortfalls {
    entries: Vec<String>,
    discarded: usize,
}

impl Shortfalls {
    fn push(&mut self, message: String) {
        if self.entries.len() < MAX_SHORTFALLS - 1 {
            self.entries.push(message);
        } else {
            self.discarded = self.discarded.saturating_add(1);
        }
    }

    fn rendered(&self) -> Vec<String> {
        let mut rendered = self.entries.clone();
        if self.discarded > 0 {
            rendered.push(format!(
                "{} relay-list failures discarded beyond the {MAX_SHORTFALLS}-entry shortfall bound",
                self.discarded
            ));
        }
        rendered
    }
}

/// NIP-65 router using explicit indexer queries for missing relay lists.
pub struct OutboxRouter {
    name: String,
    indexers: BTreeSet<RelayUrl>,
    queries: Arc<dyn QuerySource>,
}

impl OutboxRouter {
    /// Configure one NIP-65 router and the ordinary query source it uses for
    /// exact indexer acquisition.
    ///
    /// # Errors
    ///
    /// Returns [`RouterError`] when no indexer relay is configured.
    pub fn new(
        name: impl Into<String>,
        indexers: impl IntoIterator<Item = RelayUrl>,
        queries: Arc<dyn QuerySource>,
    ) -> Result<Self, RouterError> {
        let indexers: BTreeSet<_> = indexers.into_iter().collect();
        if indexers.is_empty() {
            return Err(RouterError::Refused(
                "outbox routing requires at least one indexer relay".to_owned(),
            ));
        }
        Ok(Self {
            name: name.into(),
            indexers,
            queries,
        })
    }
}

impl Router for OutboxRouter {
    fn name(&self) -> &str {
        &self.name
    }

    fn preview(
        &self,
        request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        Ok(contribution(
            request,
            &BTreeMap::new(),
            &BTreeSet::new(),
            Vec::new(),
        ))
    }

    fn open(
        &self,
        request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        let authors = requested_authors(&request);
        let queried = authors;
        let shortfalls;
        let mut settled_absent = BTreeSet::new();
        let (lists, changes) = if queried.is_empty() {
            shortfalls = Shortfalls::default();
            (BTreeMap::new(), None)
        } else {
            let query = Query::events()
                .kinds([Kind::from(10_002_u16)])
                .map_err(|error| RouterError::Refused(error.to_string()))?
                .authors(missing.iter().copied())
                .map_err(|error| RouterError::Refused(error.to_string()))?
                .from_relays(self.indexers.iter().cloned())
                .map_err(|error| RouterError::Refused(error.to_string()))?
                .with_relay_access(request.access());
            let OpenedQuerySource { initial, changes } = self
                .queries
                .open(&query)
                .map_err(|error| RouterError::Refused(error.to_string()))?;
            let parsed = decoded_lists(&initial);
            shortfalls = parsed.1;
            if settles_absence(&initial.status) {
                settled_absent.extend(queried.iter().copied());
            }
            (parsed.0, Some(changes))
        };
        Ok(Box::new(OutboxSession {
            request,
            lists,
            changes,
            queried,
            settled_absent,
            shortfalls,
            closed: false,
        }))
    }
}

struct OutboxSession {
    request: RouteRequest,
    lists: BTreeMap<PublicKey, RelayList>,
    changes: Option<Box<dyn SourceChanges>>,
    queried: BTreeSet<PublicKey>,
    settled_absent: BTreeSet<PublicKey>,
    shortfalls: Shortfalls,
    closed: bool,
}

impl OutboxSession {
    fn unresolved_count(&self) -> usize {
        self.queried
            .iter()
            .filter(|author| !self.lists.contains_key(author))
            .count()
    }

    fn contribution(&self) -> RouteContribution {
        contribution(
            &self.request,
            &self.lists,
            &self.settled_absent,
            self.shortfalls.rendered(),
        )
    }
}

impl RouterSession for OutboxSession {
    fn current(&self) -> RouteContribution {
        self.contribution()
    }

    fn next_change(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<RouteContribution, RouterError>> + Send + '_>> {
        Box::pin(async move {
            if self.closed {
                return Err(RouterError::Closed);
            }
            let changed = next_source(&mut self.changes).await;
            // Settled absence comes only from a source that reports it
            // completed; a lost source is a shortfall, not a fact.
            match changed {
                Ok(snapshot) => {
                    let parsed = decoded_lists(&snapshot);
                    self.lists = parsed.0;
                    self.shortfalls = parsed.1;
                    if settles_absence(&snapshot.status) {
                        self.settled_absent.extend(self.queried.iter().copied());
                        self.changes = None;
                    }
                }
                Err(closed) => {
                    // Termination reaches this consumer on the error
                    // channel, never as a trailing `Ok`. The cause it
                    // carries is what separates "the indexers had
                    // nothing" from "we lost the indexers".
                    self.changes = None;
                    if settles_absence(&closed.status()) {
                        self.settled_absent.extend(self.queried.iter().copied());
                    } else {
                        self.shortfalls.push(format!(
                                    "relay-list discovery source ended before settling ({}); {} author relay lists remain unresolved",
                                    closed.cause,
                                    self.unresolved_count()
                                ));
                    }
                }
            }
            Ok(self.contribution())
        })
    }

    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            if let Some(changes) = &mut self.changes {
                changes.close();
            }
        }
    }
}

fn decoded_lists(snapshot: &SourceSnapshot) -> (BTreeMap<PublicKey, RelayList>, Shortfalls) {
    let mut values = BTreeMap::new();
    let mut shortfalls = Shortfalls::default();
    for source in &snapshot.events {
        let event = match source {
            SourceEvent::Relay(relay) => EventValue::Signed(relay.event().clone()),
            SourceEvent::Local(local) => local.event.clone(),
        };
        match RelayList::from_event(&event) {
            Ok(list) => {
                values.insert(list.author(), list);
            }
            Err(error) => shortfalls.push(error.to_string()),
        }
    }
    (values, shortfalls)
}

async fn next_source(
    changes: &mut Option<Box<dyn SourceChanges>>,
) -> Result<SourceSnapshot, QuerySourceClosed> {
    match changes {
        Some(changes) => changes.next_change().await,
        None => std::future::pending().await,
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
    settled_absent: &BTreeSet<PublicKey>,
    shortfalls: Vec<String>,
) -> RouteContribution {
    let mut destinations = Vec::new();
    let mut coverage = BTreeMap::new();
    for target in request.targets() {
        let (author, relays, reason) = match &target {
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
            let state = if settled_absent.contains(author) {
                CoverageState::SettledAbsent
            } else {
                CoverageState::Unresolved
            };
            coverage.insert(target, state);
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
    RouteContribution {
        destinations,
        coverage,
        unresolved: BTreeSet::new(),
        shortfalls,
    }
}

/// Only a clean provider close settles absence.
///
/// A source that ends because the provider *failed* proves nothing about
/// whether a relay list exists; treating it as settled absence is the defect
/// this crate was corrected for. `SourceTerminationCause` makes the
/// distinction expressible, so the check is now on the cause rather than on
/// the mere fact of closure.
fn settles_absence(status: &SourceStatus) -> bool {
    matches!(
        status,
        SourceStatus::Closed {
            cause: SourceTerminationCause::ProviderClosed
        }
    )
}
