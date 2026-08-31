//! Bounded, current, typed facts published by Fava owners.
//!
//! Five categories and nothing else (`ARCH:2335-2341`): the relay sessions Fava
//! holds, the ownership graph of open observations, unsettled writes, authorized
//! provider operations, and the bounds that fell short. No policy, no health
//! score, no aggregation that turns facts into a verdict (`ARCH:2316`,
//! `GOALS:1398`).
//!
//! The bound is two-dimensional: at most `capacity` facts per category *and* at
//! most [`BoundedText::MAX_BYTES`] of externally-supplied text per string, so
//! retention is a real number of bytes (`GOALS:1439-1448`, OPS-004). Whatever the
//! count bound discards is counted in [`DroppedFacts`] rather than silently lost
//! (`GOALS:1448`).
//!
//! Every owner publishes its own facts (`ARCH:2320`); [`Diagnostics`] is
//! `Send + Sync` and every publish method takes the owned fact by value.

mod limits;
mod providers;
mod queries;
mod relays;
mod writes;

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::{Mutex, MutexGuard};

pub use fava_query::{
    BoundedText, ObservationId, QueryBounds, QueryBranchId, RelaySourceState, Round,
};
use fava_relay::RelaySessionKey;

pub use crate::limits::{BoundKind, LimitDiagnostic, LimitScope};
pub use crate::providers::{
    ProviderDiagnostic, ProviderKind, ProviderOperation, ProviderOperationState,
};
pub use crate::queries::{LogicalDemandDiagnostic, ObservationWireBinding, QueryDiagnostic};
pub use crate::relays::{RelayDiagnostic, RelaySessionState, WireSubscriptionDiagnostic};
pub use crate::writes::{WriteDiagnostic, WriteStall};

/// Bounded exact current facts published by Fava owners.
///
/// Authority: ARCH:2335-2341, verbatim five-category shape.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticsSnapshot {
    /// One entry per relay session Fava currently holds.
    pub relays: Vec<RelayDiagnostic>,
    /// One entry per open observation. This is the ownership graph.
    pub queries: Vec<QueryDiagnostic>,
    /// One entry per write that is not settled.
    pub writes: Vec<WriteDiagnostic>,
    /// One entry per provider currently executing or recently failed.
    pub providers: Vec<ProviderDiagnostic>,
    /// One entry per bound that refused, backpressured, or fell short.
    pub limits: Vec<LimitDiagnostic>,
    /// Facts dropped by the per-category count bound since construction.
    /// A bound that discards must say so (GOALS:1448).
    pub dropped_facts: DroppedFacts,
}

/// Per-category count of facts the bound discarded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DroppedFacts {
    /// Relay facts dropped.
    pub relays: u64,
    /// Query facts dropped.
    pub queries: u64,
    /// Write facts dropped.
    pub writes: u64,
    /// Provider facts dropped.
    pub providers: u64,
    /// Limit facts dropped.
    pub limits: u64,
}

/// Bounded owner of current public diagnostic facts.
///
/// Shared by every owner: `fava-observe`, `fava-transport`, `fava-publication`,
/// and `fava-runtime` all publish into one instance concurrently.
pub struct Diagnostics {
    capacity: NonZeroUsize,
    state: Mutex<State>,
}

/// One bounded fact queue per category: relays, queries, writes, providers, and
/// limits.
#[derive(Default)]
struct State {
    relays: Category<RelayDiagnostic>,
    queries: Category<QueryDiagnostic>,
    writes: Category<WriteDiagnostic>,
    providers: Category<ProviderDiagnostic>,
    limits: Category<LimitDiagnostic>,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::bounded(NonZeroUsize::new(256).expect("constant is non-zero"))
    }
}

impl Diagnostics {
    /// Construct diagnostics retaining at most `capacity` facts per category.
    #[must_use]
    pub fn bounded(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            state: Mutex::new(State::default()),
        }
    }

    /// Return one immutable current snapshot.
    #[must_use]
    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        let state = self.lock();
        DiagnosticsSnapshot {
            relays: state.relays.facts(),
            queries: state.queries.facts(),
            writes: state.writes.facts(),
            providers: state.providers.facts(),
            limits: state.limits.facts(),
            dropped_facts: DroppedFacts {
                relays: state.relays.dropped,
                queries: state.queries.dropped,
                writes: state.writes.dropped,
                providers: state.providers.dropped,
                limits: state.limits.dropped,
            },
        }
    }

    /// Publish the current facts for one relay session, replacing any earlier
    /// record for the same session.
    pub fn relay(&self, fact: RelayDiagnostic) {
        let capacity = self.capacity.get();
        let key = fact.session.clone();
        self.lock()
            .relays
            .publish(capacity, fact, |current| current.session == key);
    }

    /// Drop the record for one relay session Fava no longer holds.
    pub fn forget_relay(&self, session: &RelaySessionKey) {
        self.lock()
            .relays
            .forget(|current| &current.session == session);
    }

    /// Publish the current ownership record for one open observation,
    /// replacing any earlier record for the same observation.
    pub fn query(&self, fact: QueryDiagnostic) {
        let capacity = self.capacity.get();
        let key = fact.observation;
        self.lock()
            .queries
            .publish(capacity, fact, |current| current.observation == key);
    }

    /// Drop the record for one observation that is no longer open.
    pub fn forget_query(&self, observation: ObservationId) {
        self.lock()
            .queries
            .forget(|current| current.observation == observation);
    }

    /// Publish the current facts for one unsettled write, replacing any earlier
    /// record for the same receipt.
    pub fn write(&self, fact: WriteDiagnostic) {
        let capacity = self.capacity.get();
        let key = fact.receipt.clone();
        self.lock()
            .writes
            .publish(capacity, fact, |current| current.receipt == key);
    }

    /// Drop the record for one write that has settled.
    pub fn forget_write(&self, receipt: &BoundedText) {
        self.lock()
            .writes
            .forget(|current| &current.receipt == receipt);
    }

    /// Publish the current disposition of one provider operation, replacing any
    /// earlier record for the same provider instance.
    pub fn provider(&self, fact: ProviderDiagnostic) {
        let capacity = self.capacity.get();
        let kind = fact.provider;
        let instance = fact.operation.instance.clone();
        self.lock().providers.publish(capacity, fact, |current| {
            current.provider == kind && current.operation.instance == instance
        });
    }

    /// Drop the record for one provider instance Fava no longer holds.
    pub fn forget_provider(&self, provider: ProviderKind, instance: &BoundedText) {
        self.lock().providers.forget(|current| {
            current.provider == provider && &current.operation.instance == instance
        });
    }

    /// Publish one bound that refused, backpressured, or fell short, replacing
    /// any earlier shortfall for the same bound in the same scope.
    pub fn limit(&self, fact: LimitDiagnostic) {
        let capacity = self.capacity.get();
        let bound = fact.bound;
        let scope = fact.scope.clone();
        self.lock().limits.publish(capacity, fact, |current| {
            current.bound == bound && current.scope == scope
        });
    }

    /// Drop the shortfall record for one bound in one scope.
    pub fn forget_limit(&self, bound: BoundKind, scope: &LimitScope) {
        self.lock()
            .limits
            .forget(|current| current.bound == bound && &current.scope == scope);
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// One category's bounded retention and its honest discard count.
struct Category<T> {
    facts: VecDeque<T>,
    dropped: u64,
}

impl<T> Default for Category<T> {
    fn default() -> Self {
        Self {
            facts: VecDeque::new(),
            dropped: 0,
        }
    }
}

impl<T: Clone> Category<T> {
    /// Replace the fact with the same identity, or retain a new one, evicting
    /// the oldest and counting the eviction when the count bound is reached.
    fn publish(&mut self, capacity: usize, fact: T, same: impl Fn(&T) -> bool) {
        if let Some(index) = self.facts.iter().position(&same) {
            self.facts[index] = fact;
            return;
        }
        if self.facts.len() >= capacity {
            self.facts.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.facts.push_back(fact);
    }

    /// Remove every fact with the given identity. Deliberate removal is not a
    /// drop and does not advance `dropped`.
    fn forget(&mut self, same: impl Fn(&T) -> bool) {
        self.facts.retain(|current| !same(current));
    }

    fn facts(&self) -> Vec<T> {
        self.facts.iter().cloned().collect()
    }
}
