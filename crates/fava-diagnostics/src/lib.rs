//! Bounded current relay, subscription, and query facts.

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::{Mutex, MutexGuard};

use fava_state::RelaySessionKey;
use nostr::message::SubscriptionId;

type SessionFact = (RelaySessionKey, u64);
type SubscriptionFact = (RelaySessionKey, u64, SubscriptionId);
type MessageFact = (RelaySessionKey, u64, SubscriptionId, String);
type FailureFact = (RelaySessionKey, u64, String);
type IdentityFact = (RelaySessionKey, u64, String);

/// Bounded exact relay facts currently exposed by Fava.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticsSnapshot {
    /// Intermediate current-query revisions intentionally superseded by newer state.
    pub coalesced_query_updates: u64,
    /// Automatic router instances opened by live queries.
    pub router_sessions: Vec<String>,
    /// Exact relay destinations in recent live route revisions.
    pub routes: Vec<(u64, Vec<RelaySessionKey>)>,
    /// Exact routing failures or limits by route revision.
    pub route_shortfalls: Vec<(u64, String)>,
    /// Most recently opened exact relay-session generations.
    pub sessions: Vec<SessionFact>,
    /// Most recently opened exact Nostr subscriptions.
    pub subscriptions: Vec<SubscriptionFact>,
    /// Actual EOSE frames attributed to exact subscriptions.
    pub eose: Vec<SubscriptionFact>,
    /// Actual CLOSED frames and relay messages.
    pub closed: Vec<MessageFact>,
    /// Exact sessions that supplied an AUTH challenge.
    pub authentication_required: Vec<SessionFact>,
    /// Exact sessions authenticated, with the authorized identity.
    pub authenticated: Vec<IdentityFact>,
    /// Exact sessions whose authentication was declined, refused, or failed.
    pub authentication_denied: Vec<FailureFact>,
    /// Limits each relay currently declares, or why they stay unknown.
    pub relay_limits: Vec<(RelaySessionKey, String)>,
    /// Exact refusals produced before knowingly invalid work was sent.
    pub relay_limit_shortfalls: Vec<(RelaySessionKey, String)>,
    /// Exact scoped session failures.
    pub failures: Vec<FailureFact>,
    /// Exact subscriptions withdrawn locally with CLOSE.
    pub withdrawn: Vec<SubscriptionFact>,
}

/// Bounded owner of current public diagnostic facts.
pub struct Diagnostics {
    capacity: NonZeroUsize,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    coalesced_query_updates: u64,
    router_sessions: VecDeque<String>,
    routes: VecDeque<(u64, Vec<RelaySessionKey>)>,
    route_shortfalls: VecDeque<(u64, String)>,
    sessions: VecDeque<SessionFact>,
    subscriptions: VecDeque<SubscriptionFact>,
    eose: VecDeque<SubscriptionFact>,
    closed: VecDeque<MessageFact>,
    authentication_required: VecDeque<SessionFact>,
    authenticated: VecDeque<IdentityFact>,
    authentication_denied: VecDeque<FailureFact>,
    relay_limits: VecDeque<(RelaySessionKey, String)>,
    relay_limit_shortfalls: VecDeque<(RelaySessionKey, String)>,
    failures: VecDeque<FailureFact>,
    withdrawn: VecDeque<SubscriptionFact>,
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
            coalesced_query_updates: state.coalesced_query_updates,
            router_sessions: state.router_sessions.iter().cloned().collect(),
            routes: state.routes.iter().cloned().collect(),
            route_shortfalls: state.route_shortfalls.iter().cloned().collect(),
            sessions: state.sessions.iter().cloned().collect(),
            subscriptions: state.subscriptions.iter().cloned().collect(),
            eose: state.eose.iter().cloned().collect(),
            closed: state.closed.iter().cloned().collect(),
            authentication_required: state.authentication_required.iter().cloned().collect(),
            authenticated: state.authenticated.iter().cloned().collect(),
            authentication_denied: state.authentication_denied.iter().cloned().collect(),
            relay_limits: state.relay_limits.iter().cloned().collect(),
            relay_limit_shortfalls: state.relay_limit_shortfalls.iter().cloned().collect(),
            failures: state.failures.iter().cloned().collect(),
            withdrawn: state.withdrawn.iter().cloned().collect(),
        }
    }

    /// Record current-query revisions superseded before delivery.
    pub fn query_updates_coalesced(&self, count: u64) {
        let mut state = self.lock();
        state.coalesced_query_updates = state.coalesced_query_updates.saturating_add(count);
    }

    /// Record one automatic router-instance open.
    pub fn router_opened(&self, name: String) {
        let capacity = self.capacity.get();
        push_bounded(&mut self.lock().router_sessions, capacity, name);
    }

    /// Record the exact destinations in one current route revision.
    pub fn route(&self, revision: u64, relays: Vec<RelaySessionKey>) {
        let capacity = self.capacity.get();
        push_bounded(&mut self.lock().routes, capacity, (revision, relays));
    }

    /// Record one exact route refusal or limit.
    pub fn route_shortfall(&self, revision: u64, message: String) {
        let capacity = self.capacity.get();
        push_bounded(
            &mut self.lock().route_shortfalls,
            capacity,
            (revision, message),
        );
    }

    /// Record one opened relay-session generation.
    pub fn session_opened(&self, session: RelaySessionKey, generation: u64) {
        let capacity = self.capacity.get();
        push_bounded(&mut self.lock().sessions, capacity, (session, generation));
    }

    /// Record one accepted Nostr subscription.
    pub fn subscription_opened(
        &self,
        session: RelaySessionKey,
        generation: u64,
        subscription: SubscriptionId,
    ) {
        let capacity = self.capacity.get();
        push_bounded(
            &mut self.lock().subscriptions,
            capacity,
            (session, generation, subscription),
        );
    }

    /// Record one actual EOSE frame.
    pub fn eose(&self, session: RelaySessionKey, generation: u64, subscription: SubscriptionId) {
        let capacity = self.capacity.get();
        push_bounded(
            &mut self.lock().eose,
            capacity,
            (session, generation, subscription),
        );
    }

    /// Record one actual CLOSED frame.
    pub fn closed(
        &self,
        session: RelaySessionKey,
        generation: u64,
        subscription: SubscriptionId,
        message: String,
    ) {
        let capacity = self.capacity.get();
        push_bounded(
            &mut self.lock().closed,
            capacity,
            (session, generation, subscription, message),
        );
    }

    /// Record one session-scoped AUTH challenge.
    pub fn authentication_required(&self, session: RelaySessionKey, generation: u64) {
        let capacity = self.capacity.get();
        push_bounded(
            &mut self.lock().authentication_required,
            capacity,
            (session, generation),
        );
    }

    /// Record one accepted relay authentication and its authorized identity.
    pub fn authenticated(&self, session: RelaySessionKey, generation: u64, identity: String) {
        let capacity = self.capacity.get();
        push_bounded(
            &mut self.lock().authenticated,
            capacity,
            (session, generation, identity),
        );
    }

    /// Record one declined, refused, or failed relay authentication.
    pub fn authentication_denied(&self, session: RelaySessionKey, generation: u64, reason: String) {
        let capacity = self.capacity.get();
        push_bounded(
            &mut self.lock().authentication_denied,
            capacity,
            (session, generation, reason),
        );
    }

    /// Record the limits one relay currently declares, or why they are unknown.
    pub fn relay_limits(&self, session: RelaySessionKey, declared: String) {
        let capacity = self.capacity.get();
        push_bounded(&mut self.lock().relay_limits, capacity, (session, declared));
    }

    /// Record one exact refusal produced before knowingly invalid work was sent.
    pub fn relay_limit_shortfall(&self, session: RelaySessionKey, reason: String) {
        let capacity = self.capacity.get();
        push_bounded(
            &mut self.lock().relay_limit_shortfalls,
            capacity,
            (session, reason),
        );
    }

    /// Record one scoped transport or protocol failure.
    pub fn failed(&self, session: RelaySessionKey, generation: u64, message: String) {
        let capacity = self.capacity.get();
        push_bounded(
            &mut self.lock().failures,
            capacity,
            (session, generation, message),
        );
    }

    /// Record one locally withdrawn exact subscription.
    pub fn withdrawn(
        &self,
        session: RelaySessionKey,
        generation: u64,
        subscription: SubscriptionId,
    ) {
        let capacity = self.capacity.get();
        push_bounded(
            &mut self.lock().withdrawn,
            capacity,
            (session, generation, subscription),
        );
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn push_bounded<T: Eq>(queue: &mut VecDeque<T>, capacity: usize, value: T) {
    if let Some(index) = queue.iter().position(|current| current == &value) {
        queue.remove(index);
    }
    if queue.len() == capacity {
        queue.pop_front();
    }
    queue.push_back(value);
}
