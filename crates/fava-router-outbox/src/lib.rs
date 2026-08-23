//! NIP-65 author-outbox and recipient-inbox routing.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use fava_nip65::{RelayList, RelayListError};
use fava_query::{SourceTerminationCause, 
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, SourceChanges, SourceEvent,
    SourceSnapshot, SourceStatus,
};
use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, RouteTarget,
    Router, RouterError, RouterSession,
};
use fava_state::{PublicKey, RelaySessionKey, RelayUrl};
use fava_write::{EventValue, Kind};
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
    lists: Arc<KnownLists>,
}

struct KnownLists {
    values: Mutex<BTreeMap<PublicKey, RelayList>>,
    revision: watch::Sender<u64>,
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
        let (revision, _) = watch::channel(0);
        Ok(Self {
            name: name.into(),
            indexers,
            queries,
            lists: Arc::new(KnownLists {
                values: Mutex::new(BTreeMap::new()),
                revision,
            }),
        })
    }

    /// Add one locally known NIP-65 event when it supersedes current knowledge.
    ///
    /// # Errors
    ///
    /// Returns [`RelayListError`] when the event is not a valid relay list.
    pub fn remember(&self, event: &EventValue) -> Result<bool, RelayListError> {
        self.lists.remember(event)
    }

    fn contribution(
        &self,
        request: &RouteRequest,
        settled_absent: &BTreeSet<PublicKey>,
        shortfalls: &Shortfalls,
    ) -> RouteContribution {
        contribution(
            request,
            &self.lists.values(),
            settled_absent,
            shortfalls.rendered(),
        )
    }
}

impl KnownLists {
    fn values(&self) -> BTreeMap<PublicKey, RelayList> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn remember(&self, event: &EventValue) -> Result<bool, RelayListError> {
        let candidate = RelayList::from_event(event)?;
        let mut values = self
            .values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let replace = values
            .get(&candidate.author())
            .is_none_or(|current| candidate.supersedes(current));
        if !replace {
            return Ok(false);
        }
        values.insert(candidate.author(), candidate);
        drop(values);
        let next = self.revision.borrow().saturating_add(1);
        self.revision.send_replace(next);
        Ok(true)
    }

    fn ingest(&self, snapshot: &SourceSnapshot, shortfalls: &mut Shortfalls) {
        for source in &snapshot.events {
            let event = match source {
                SourceEvent::Cached(cached) => EventValue::Signed(cached.event.clone()),
                SourceEvent::Local(local) => local.event.clone(),
            };
            if let Err(error) = self.remember(&event) {
                shortfalls.push(error.to_string());
            }
        }
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
        Ok(self.contribution(request, &BTreeSet::new(), &Shortfalls::default()))
    }

    fn open(
        &self,
        request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        let authors = requested_authors(&request);
        let known = self.lists.values();
        let missing: BTreeSet<_> = authors
            .into_iter()
            .filter(|author| !known.contains_key(author))
            .collect();
        let mut shortfalls = Shortfalls::default();
        let mut settled_absent = BTreeSet::new();
        let changes = if missing.is_empty() {
            None
        } else {
            let query = Query::events()
                .kind(Kind::from(10_002_u16))
                .authors(missing.iter().copied())
                .from_relays(self.indexers.iter().cloned())
                .map_err(|error| RouterError::Refused(error.to_string()))?;
            let OpenedQuerySource { initial, changes } = self
                .queries
                .open(&query)
                .map_err(|error| RouterError::Refused(error.to_string()))?;
            self.lists.ingest(&initial, &mut shortfalls);
            if settles_absence(&initial.status) {
                settled_absent.extend(missing.iter().copied());
            }
            Some(changes)
        };
        Ok(Box::new(OutboxSession {
            request,
            lists: Arc::clone(&self.lists),
            revision: self.lists.revision.subscribe(),
            changes,
            queried: missing,
            settled_absent,
            shortfalls,
            closed: false,
        }))
    }
}

struct OutboxSession {
    request: RouteRequest,
    lists: Arc<KnownLists>,
    revision: watch::Receiver<u64>,
    changes: Option<Box<dyn SourceChanges>>,
    queried: BTreeSet<PublicKey>,
    settled_absent: BTreeSet<PublicKey>,
    shortfalls: Shortfalls,
    closed: bool,
}

impl OutboxSession {
    fn unresolved_count(&self) -> usize {
        let known = self.lists.values();
        self.queried
            .iter()
            .filter(|author| !known.contains_key(author))
            .count()
    }

    fn contribution(&self) -> RouteContribution {
        contribution(
            &self.request,
            &self.lists.values(),
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
            tokio::select! {
                changed = self.revision.changed() => {
                    changed.map_err(|_| RouterError::Closed)?;
                    self.revision.borrow_and_update();
                }
                changed = next_source(&mut self.changes) => {
                    // Settled absence comes only from a source that reports it
                    // completed; a lost source is a shortfall, not a fact.
                    if let Ok(snapshot) = changed {
                        self.lists.ingest(&snapshot, &mut self.shortfalls);
                        if settles_absence(&snapshot.status) {
                            self.settled_absent.extend(self.queried.iter().copied());
                            self.changes = None;
                        }
                    } else {
                        self.changes = None;
                        self.shortfalls.push(format!(
                            "relay-list discovery source closed before settling; {} author relay lists remain unresolved",
                            self.unresolved_count()
                        ));
                    }
                    self.revision.borrow_and_update();
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
            .map(|relay| RelaySessionKey::new(relay, request.access()))
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
