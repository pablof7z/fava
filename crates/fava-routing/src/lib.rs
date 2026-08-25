//! Ordered asynchronous composition of relay-routing contributions.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use fava_query::Query;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_write::{EventId, EventValue};
use nostr::key::PublicKey;
use nostr::types::RelayUrl;
use thiserror::Error;
use tokio::sync::watch;

mod chain;

pub use chain::{open, preview};

/// Facts for which automatic relay destinations are requested.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteRequest {
    /// Route one event query.
    Read(Query),
    /// Route one event publication.
    Write(EventValue),
}

impl RouteRequest {
    /// Exact coverage targets implied by this request.
    #[must_use]
    pub fn targets(&self) -> BTreeSet<RouteTarget> {
        match self {
            Self::Read(query) => query
                .selection()
                .authors
                .as_ref()
                .filter(|authors| !authors.is_empty())
                .map_or_else(
                    || {
                        query
                            .selection()
                            .ids
                            .as_ref()
                            .filter(|ids| !ids.is_empty())
                            .map_or_else(
                                || BTreeSet::from([RouteTarget::WholeRequest]),
                                |ids| {
                                    ids.iter()
                                        .copied()
                                        .map(RouteTarget::ReferencedEvent)
                                        .collect()
                                },
                            )
                    },
                    |authors| authors.iter().copied().map(RouteTarget::Author).collect(),
                ),
            Self::Write(event) => {
                let mut targets = BTreeSet::from([RouteTarget::Author(event.author())]);
                for tag in event.tags() {
                    let values = tag.as_slice();
                    if values.first().map(String::as_str) == Some("p")
                        && let Some(value) = values.get(1)
                        && let Ok(public_key) = PublicKey::parse(value)
                    {
                        targets.insert(RouteTarget::Recipient(public_key));
                    }
                    if values.first().map(String::as_str) == Some("e")
                        && let Some(value) = values.get(1)
                        && let Ok(event_id) = EventId::parse(value)
                    {
                        targets.insert(RouteTarget::ReferencedEvent(event_id));
                    }
                }
                targets
            }
        }
    }

    /// Relay access under which selected destinations must execute.
    #[must_use]
    pub fn access(&self) -> RelayAccess {
        match self {
            Self::Read(query) => query.access().clone(),
            Self::Write(_) => RelayAccess::Public,
        }
    }

    /// Event facts for a write request.
    #[must_use]
    pub const fn event(&self) -> Option<&EventValue> {
        match self {
            Self::Read(_) => None,
            Self::Write(event) => Some(event),
        }
    }

    /// Whether this request routes a read.
    #[must_use]
    pub const fn is_read(&self) -> bool {
        matches!(self, Self::Read(_))
    }

    /// Whether this request routes a write.
    #[must_use]
    pub const fn is_write(&self) -> bool {
        matches!(self, Self::Write(_))
    }
}

/// One independently tracked routing-coverage target.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RouteTarget {
    /// The complete request when no narrower author target applies.
    WholeRequest,
    /// One event author selected by a read.
    Author(PublicKey),
    /// One public key tagged as a recipient of a write.
    Recipient(PublicKey),
    /// One immutable event referenced by a write.
    ReferencedEvent(EventId),
}

/// Current routing knowledge for one target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoverageState {
    /// Exact relay sessions currently covering the target.
    Covered(BTreeSet<RelaySessionKey>),
    /// Relevant routing knowledge has not settled.
    Unresolved,
    /// Relevant routing knowledge settled without a destination.
    SettledAbsent,
}

/// One relay destination supplied by one router.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteDestination {
    /// Exact relay and access identity.
    pub session: RelaySessionKey,
    /// Targets this relay covers.
    pub targets: BTreeSet<RouteTarget>,
    /// Router instance attribution, stamped by the routing core.
    router: String,
    /// Router-owned explanation for this contribution.
    pub reason: String,
}

impl RouteDestination {
    /// Contribute one relay destination. Router attribution is added by the chain.
    #[must_use]
    pub fn new(
        session: RelaySessionKey,
        targets: BTreeSet<RouteTarget>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            session,
            targets,
            router: String::new(),
            reason: reason.into(),
        }
    }

    /// Router instance that contributed this destination.
    #[must_use]
    pub fn router(&self) -> &str {
        &self.router
    }

    pub(crate) fn set_router(&mut self, router: &str) {
        router.clone_into(&mut self.router);
    }
}

/// One router's complete current replacement contribution.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RouteContribution {
    /// Current relay destinations.
    pub destinations: Vec<RouteDestination>,
    /// Current per-target coverage knowledge.
    pub coverage: BTreeMap<RouteTarget, CoverageState>,
    /// Targets for which this router still owes a later answer.
    pub unresolved: BTreeSet<RouteTarget>,
    /// Exact bounded failures or limits affecting this contribution.
    pub shortfalls: Vec<String>,
}

/// One deduplicated relay in a current route plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedRelay {
    /// Exact relay and access identity.
    pub session: RelaySessionKey,
    /// Union of targets covered at this relay.
    pub targets: BTreeSet<RouteTarget>,
    /// Exact `(router, reason)` pairs that selected this relay.
    pub reasons: BTreeSet<(String, String)>,
}

/// Complete current result of ordered routing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RoutePlan {
    /// Monotonic revision of this open routing calculation.
    pub revision: u64,
    /// Deduplicated exact relay destinations.
    pub destinations: BTreeMap<RelaySessionKey, PlannedRelay>,
    /// Merged current target coverage.
    pub coverage: BTreeMap<RouteTarget, CoverageState>,
    /// Targets for which at least one configured router still owes an answer.
    pub unresolved: BTreeSet<RouteTarget>,
    /// Exact current bounded routing failures or limits.
    pub shortfalls: Vec<String>,
}

impl RoutePlan {
    /// Whether no target remains unresolved.
    ///
    /// Derived from `unresolved`, never carried as independent state: every
    /// producer of a `RoutePlan` computes settlement from the exact same
    /// unresolved set, so a separate stored flag could disagree with it.
    #[must_use]
    pub fn settled(&self) -> bool {
        self.unresolved.is_empty()
    }

    /// Build a bounded plan preserving one routing refusal as current evidence.
    #[must_use]
    pub fn shortfall(revision: u64, request: &RouteRequest, reason: String) -> Self {
        Self {
            revision,
            destinations: BTreeMap::new(),
            coverage: request
                .targets()
                .into_iter()
                .map(|target| (target, CoverageState::Unresolved))
                .collect(),
            unresolved: request.targets(),
            shortfalls: vec![reason],
        }
    }

    /// Build one plan from a complete merged contribution.
    ///
    /// # Errors
    ///
    /// Returns [`RouterError`] when the contribution exceeds routing bounds.
    pub fn from_contribution(
        revision: u64,
        contribution: &RouteContribution,
    ) -> Result<Self, RouterError> {
        chain::validate_combined(contribution)?;
        let mut destinations = BTreeMap::<RelaySessionKey, PlannedRelay>::new();
        let mut coverage = contribution.coverage.clone();
        let mut unresolved = contribution.unresolved.clone();
        unresolved.extend(
            contribution
                .coverage
                .iter()
                .filter(|(_, state)| matches!(state, CoverageState::Unresolved))
                .map(|(target, _)| target.clone()),
        );
        for destination in &contribution.destinations {
            let planned = destinations
                .entry(destination.session.clone())
                .or_insert_with(|| PlannedRelay {
                    session: destination.session.clone(),
                    targets: BTreeSet::new(),
                    reasons: BTreeSet::new(),
                });
            planned.targets.extend(destination.targets.iter().cloned());
            planned
                .reasons
                .insert((destination.router.clone(), destination.reason.clone()));
            for target in &destination.targets {
                merge_coverage(
                    coverage
                        .entry(target.clone())
                        .or_insert(CoverageState::Unresolved),
                    &CoverageState::Covered(BTreeSet::from([destination.session.clone()])),
                );
            }
        }
        Ok(Self {
            revision,
            destinations,
            coverage,
            unresolved,
            shortfalls: contribution.shortfalls.clone(),
        })
    }

    /// Build an exact plan without opening any automatic router.
    ///
    /// # Errors
    ///
    /// Returns [`RouterError`] when the explicit relay demand exceeds routing bounds.
    pub fn explicit(
        relays: impl IntoIterator<Item = RelayUrl>,
        access: &RelayAccess,
        targets: &BTreeSet<RouteTarget>,
    ) -> Result<Self, RouterError> {
        let contribution = RouteContribution {
            destinations: relays
                .into_iter()
                .map(|relay| {
                    let mut destination = RouteDestination::new(
                        RelaySessionKey {
                            relay,
                            access: access.clone(),
                        },
                        targets.clone(),
                        "application-selected relay",
                    );
                    destination.set_router("explicit");
                    destination
                })
                .collect(),
            coverage: BTreeMap::new(),
            unresolved: BTreeSet::new(),
            shortfalls: Vec::new(),
        };
        chain::validate_router_contribution(&contribution)?;
        Self::from_contribution(1, &contribution)
    }
}

/// Independently selectable automatic relay-routing policy.
pub trait Router: Send + Sync {
    /// Stable configured router-instance name used for attribution.
    fn name(&self) -> &str;

    /// Evaluate current known facts without starting acquisition.
    ///
    /// # Errors
    ///
    /// Returns [`RouterError`] when current facts cannot be evaluated exactly.
    fn preview(
        &self,
        request: &RouteRequest,
        upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError>;

    /// Open one live complete-contribution sequence.
    ///
    /// # Errors
    ///
    /// Returns [`RouterError`] before a usable session exists.
    fn open(
        &self,
        request: RouteRequest,
        upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError>;
}

/// One exact router-instance lifecycle for one request.
pub trait RouterSession: Send {
    /// Immediate complete current contribution.
    fn current(&self) -> RouteContribution;

    /// Await a later complete replacement contribution.
    #[allow(clippy::type_complexity)]
    fn next_change(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<RouteContribution, RouterError>> + Send + '_>>;

    /// Release all work owned by this router session.
    fn close(&mut self);
}

/// Scoped refusal or termination of one router instance.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RouterError {
    /// Router or chain refused exact work.
    #[error("router refused work: {0}")]
    Refused(String),
    /// Router session ended after a coherent contribution.
    #[error("router session closed")]
    Closed,
}

pub(crate) fn merge_coverage(current: &mut CoverageState, incoming: &CoverageState) {
    match incoming {
        CoverageState::Covered(incoming) => match current {
            CoverageState::Covered(current) => current.extend(incoming.iter().cloned()),
            _ => *current = CoverageState::Covered(incoming.clone()),
        },
        CoverageState::Unresolved if !matches!(current, CoverageState::Covered(_)) => {
            *current = CoverageState::Unresolved;
        }
        CoverageState::Unresolved | CoverageState::SettledAbsent => {}
    }
}
