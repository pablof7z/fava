use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use fava_auth::{AUTHENTICATION_DEADLINE, Authentication, AuthenticationOutcome, RelayChallenge};
use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_ingest::admit_subscription_event;
use fava_nip11::{RelayInformationFetcher, RelayLimitation};
use fava_query::Query;
use fava_state::{RelaySessionKey, Timestamp};
use fava_subscriptions::{RelayLimits, SubscriptionPlan, SubscriptionPlanner, demand_for_query};
use fava_transport::{HandoffOutcome, RelaySession, Transport};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId, decode_relay, encode_client};
use nostr::event::EventId;
use nostr::filter::Filter;
use nostr::key::PublicKey;
use tokio::sync::watch;

/// Providers one live relay task needs, selected once by the assembly.
#[derive(Clone)]
pub(super) struct RelayProviders {
    pub(super) transport: Arc<dyn Transport>,
    pub(super) planner: Arc<dyn SubscriptionPlanner>,
    pub(super) cache: Arc<dyn EventCache>,
    pub(super) diagnostics: Arc<Diagnostics>,
    pub(super) next_subscription: Arc<AtomicU64>,
    pub(super) authentication: Option<Arc<Authentication>>,
    pub(super) relay_information: Option<Arc<dyn RelayInformationFetcher>>,
}

impl RelayProviders {
    /// Resolve the limits this relay currently declares.
    ///
    /// A relay that is unreachable for its information document, or that
    /// answers with something Fava cannot interpret, leaves every limit
    /// unknown. Fava's own configured bounds still apply; no relay claim is
    /// invented.
    async fn limits(&self, session_key: &RelaySessionKey) -> RelayLimits {
        let Some(fetcher) = self.relay_information.as_ref() else {
            return RelayLimits::unknown();
        };
        match fetcher.get(session_key.relay.clone()).await {
            Ok(document) => {
                let limits = document.limitation.planning();
                self.diagnostics
                    .relay_limits(session_key.clone(), describe(&document.limitation));
                limits
            }
            Err(error) => {
                self.diagnostics
                    .relay_limits(session_key.clone(), format!("unknown: {error}"));
                RelayLimits::unknown()
            }
        }
    }
}

pub(super) struct OpenedRelay {
    session_key: RelaySessionKey,
    query: Query,
    providers: RelayProviders,
    session: Arc<dyn RelaySession>,
    attribution: BTreeMap<SubscriptionId, Filter>,
    demand: Vec<ClientMessage<'static>>,
    pending_authentication: Option<(EventId, PublicKey)>,
}

impl OpenedRelay {
    pub(super) async fn open(
        session_key: RelaySessionKey,
        query: Query,
        providers: RelayProviders,
    ) -> Result<Self, String> {
        let established = establish(&session_key, &query, &providers).await?;
        Ok(Self {
            session_key,
            query,
            providers,
            session: established.session,
            attribution: established.attribution,
            demand: established.demand,
            pending_authentication: None,
        })
    }

    pub(super) async fn abort(self) {
        withdraw(
            self.session.as_ref(),
            self.providers.diagnostics.as_ref(),
            &self.attribution,
        )
        .await;
    }

    pub(super) async fn run(mut self, mut cancel: watch::Receiver<bool>) {
        loop {
            tokio::select! {
                biased;
                changed = cancel.changed() => {
                    if changed.is_err() || *cancel.borrow_and_update() {
                        withdraw(
                            self.session.as_ref(),
                            self.providers.diagnostics.as_ref(),
                            &self.attribution,
                        ).await;
                        return;
                    }
                }
                inbound = self.session.next_message() => {
                    match inbound {
                        Ok(frame) => self.handle_frame(&frame).await,
                        Err(error) => {
                            self.providers.diagnostics.failed(
                                self.session_key.clone(),
                                self.session.generation(),
                                error.to_string(),
                            );
                            if !self.reconnect(&mut cancel).await {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn handle_frame(&mut self, frame: &str) {
        let generation = self.session.generation();
        let message = match decode_relay(frame) {
            Ok(message) => message,
            Err(error) => {
                self.providers.diagnostics.failed(
                    self.session_key.clone(),
                    generation,
                    format!("invalid relay message: {error}"),
                );
                return;
            }
        };
        match message {
            RelayMessage::Auth { challenge } => {
                self.answer_challenge(challenge.into_owned()).await;
            }
            RelayMessage::Ok {
                event_id,
                status,
                message,
            } => {
                self.settle_authentication(event_id, status, message.into_owned())
                    .await;
            }
            message => handle_message(
                self.session.as_ref(),
                self.providers.cache.as_ref(),
                self.providers.diagnostics.as_ref(),
                &self.attribution,
                message,
            ),
        }
    }

    /// Answer one exact challenge on the generation that carried it.
    ///
    /// The application policy decides. A decline or failure ends only this
    /// relay session's authentication; the query, other relays, and other
    /// relay-access identities are untouched.
    async fn answer_challenge(&mut self, challenge: String) {
        let generation = self.session.generation();
        self.providers
            .diagnostics
            .authentication_required(self.session_key.clone(), generation);
        let Some(authentication) = self.providers.authentication.clone() else {
            return;
        };
        let challenge = match RelayChallenge::new(self.session_key.clone(), generation, challenge) {
            Ok(challenge) => challenge,
            Err(error) => {
                self.providers.diagnostics.authentication_denied(
                    self.session_key.clone(),
                    generation,
                    error.to_string(),
                );
                return;
            }
        };
        let prepared = match tokio::time::timeout(
            AUTHENTICATION_DEADLINE,
            authentication.prepare(&challenge, self.session.as_ref()),
        )
        .await
        {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(outcome)) => {
                self.providers.diagnostics.authentication_denied(
                    self.session_key.clone(),
                    generation,
                    denial_reason(&outcome),
                );
                return;
            }
            Err(_) => {
                self.providers.diagnostics.authentication_denied(
                    self.session_key.clone(),
                    generation,
                    "relay authentication decision exceeded its deadline".to_owned(),
                );
                return;
            }
        };
        match self.session.send(prepared.frame).await {
            HandoffOutcome::HandedOff => {
                self.pending_authentication = Some((prepared.answer, prepared.identity));
            }
            HandoffOutcome::NotHandedOff { reason } | HandoffOutcome::Ambiguous { reason } => {
                self.providers.diagnostics.authentication_denied(
                    self.session_key.clone(),
                    generation,
                    reason,
                );
            }
        }
    }

    /// Settle a pending authentication and restore demand it unblocked.
    async fn settle_authentication(&mut self, event_id: EventId, status: bool, message: String) {
        let Some((answer, identity)) = self.pending_authentication else {
            return;
        };
        if answer != event_id {
            return;
        }
        self.pending_authentication = None;
        let generation = self.session.generation();
        match Authentication::settle(identity, status, message) {
            AuthenticationOutcome::Accepted { identity, .. } => {
                self.providers.diagnostics.authenticated(
                    self.session_key.clone(),
                    generation,
                    identity.to_hex(),
                );
                self.restore_demand().await;
            }
            outcome => self.providers.diagnostics.authentication_denied(
                self.session_key.clone(),
                generation,
                denial_reason(&outcome),
            ),
        }
    }

    /// Re-issue the exact accepted plan after authentication unblocks it.
    async fn restore_demand(&self) {
        for message in &self.demand {
            let Ok(frame) = encode_client(message) else {
                continue;
            };
            if let HandoffOutcome::NotHandedOff { reason } | HandoffOutcome::Ambiguous { reason } =
                self.session.send(frame).await
            {
                self.providers.diagnostics.failed(
                    self.session_key.clone(),
                    self.session.generation(),
                    format!("authenticated demand was not restored: {reason}"),
                );
            }
        }
    }

    async fn reconnect(&mut self, cancel: &mut watch::Receiver<bool>) -> bool {
        loop {
            tokio::select! {
                biased;
                changed = cancel.changed() => {
                    if changed.is_err() || *cancel.borrow_and_update() {
                        return false;
                    }
                }
                () = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
            let reconnect = establish(&self.session_key, &self.query, &self.providers);
            let established = tokio::select! {
                biased;
                changed = cancel.changed() => {
                    if changed.is_err() || *cancel.borrow_and_update() {
                        return false;
                    }
                    continue;
                }
                established = reconnect => established,
            };
            match established {
                Ok(established) => {
                    self.session = established.session;
                    self.attribution = established.attribution;
                    self.demand = established.demand;
                    self.pending_authentication = None;
                    return true;
                }
                Err(error) => self.providers.diagnostics.failed(
                    self.session_key.clone(),
                    self.session.generation(),
                    format!("reconnect refused: {error}"),
                ),
            }
        }
    }
}

/// One accepted relay session and the exact demand it carries.
struct Established {
    session: Arc<dyn RelaySession>,
    attribution: BTreeMap<SubscriptionId, Filter>,
    demand: Vec<ClientMessage<'static>>,
}

async fn establish(
    session_key: &RelaySessionKey,
    query: &Query,
    providers: &RelayProviders,
) -> Result<Established, String> {
    let diagnostics = providers.diagnostics.as_ref();
    let limits = providers.limits(session_key).await;
    let subscription = allocate_subscription(providers.next_subscription.as_ref())?;
    let plan = providers
        .planner
        .plan(
            session_key,
            &limits,
            &[demand_for_query(subscription, query)],
        )
        .map_err(|error| {
            diagnostics.relay_limit_shortfall(session_key.clone(), error.to_string());
            error.to_string()
        })?;
    validate_plan(session_key, &plan)?;
    let session = providers
        .transport
        .open_session(session_key.clone())
        .await
        .map_err(|error| error.to_string())?;
    if session.key() != session_key {
        let _ = session.close().await;
        return Err("transport returned the wrong relay session identity".to_owned());
    }
    let generation = session.generation();
    diagnostics.session_opened(session_key.clone(), generation);
    for message in &plan.messages {
        let frame = encode_client(message).map_err(|error| error.to_string())?;
        match session.send(frame).await {
            HandoffOutcome::HandedOff => {}
            HandoffOutcome::NotHandedOff { reason } => {
                let _ = session.close().await;
                return Err(format!("subscription was not handed off: {reason}"));
            }
            HandoffOutcome::Ambiguous { reason } => {
                let _ = session.close().await;
                return Err(format!("subscription handoff is ambiguous: {reason}"));
            }
        }
    }
    for id in plan.attribution.keys() {
        diagnostics.subscription_opened(session_key.clone(), generation, id.clone());
    }
    Ok(Established {
        session,
        attribution: plan.attribution,
        demand: plan.messages,
    })
}

/// Exact reason one authentication did not authorize relay access.
fn denial_reason(outcome: &AuthenticationOutcome) -> String {
    match outcome {
        AuthenticationOutcome::Accepted { .. } => "authenticated".to_owned(),
        AuthenticationOutcome::Refused { message } => {
            format!("relay refused authentication: {message}")
        }
        AuthenticationOutcome::Declined { reason } => {
            format!("application declined authentication: {reason}")
        }
        AuthenticationOutcome::Failed { reason } => format!("authentication failed: {reason}"),
    }
}

fn allocate_subscription(next: &AtomicU64) -> Result<SubscriptionId, String> {
    let sequence = next
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(1)
        })
        .map_err(|_| "subscription identity exhausted".to_owned())?
        + 1;
    Ok(SubscriptionId::new(format!("fava-{sequence}")))
}

fn validate_plan(expected: &RelaySessionKey, plan: &SubscriptionPlan) -> Result<(), String> {
    if &plan.relay != expected
        || plan.attribution.is_empty()
        || plan.messages.is_empty()
        || !plan.demand.keys().eq(plan.attribution.keys())
        || plan.demand.values().any(Vec::is_empty)
    {
        return Err("subscription planner returned incomplete or mis-scoped work".to_owned());
    }
    for message in &plan.messages {
        let ClientMessage::Req {
            subscription_id,
            filters,
        } = message
        else {
            return Err("subscription planner returned a non-REQ message".to_owned());
        };
        if filters.len() != 1
            || plan.attribution.get(subscription_id.as_ref()) != Some(filters[0].as_ref())
        {
            return Err("subscription planner attribution does not match its REQ".to_owned());
        }
    }
    Ok(())
}

fn handle_message(
    session: &dyn RelaySession,
    cache: &dyn EventCache,
    diagnostics: &Diagnostics,
    attribution: &BTreeMap<SubscriptionId, Filter>,
    message: RelayMessage<'static>,
) {
    let key = session.key().clone();
    let generation = session.generation();
    match message {
        RelayMessage::Event {
            subscription_id,
            event,
        } => {
            let id = subscription_id.into_owned();
            let Some(filter) = attribution.get(&id) else {
                diagnostics.failed(key, generation, format!("unattributed EVENT for {id}"));
                return;
            };
            if let Err(error) = admit_subscription_event(
                cache,
                session.key(),
                &id,
                &id,
                filter,
                event.into_owned(),
                Timestamp::now(),
            ) {
                diagnostics.failed(key, generation, error.to_string());
            }
        }
        RelayMessage::EndOfStoredEvents(subscription) => {
            let id = subscription.into_owned();
            if attribution.contains_key(&id) {
                diagnostics.eose(key, generation, id);
            } else {
                diagnostics.failed(key, generation, format!("unattributed EOSE for {id}"));
            }
        }
        RelayMessage::Closed {
            subscription_id,
            message,
        } => {
            let id = subscription_id.into_owned();
            if attribution.contains_key(&id) {
                diagnostics.closed(key, generation, id, message.into_owned());
            } else {
                diagnostics.failed(key, generation, format!("unattributed CLOSED for {id}"));
            }
        }
        RelayMessage::Notice(message) => {
            diagnostics.failed(key, generation, format!("relay NOTICE: {message}"));
        }
        RelayMessage::Auth { .. }
        | RelayMessage::Ok { .. }
        | RelayMessage::Count { .. }
        | RelayMessage::NegMsg { .. }
        | RelayMessage::NegErr { .. } => {}
    }
}

async fn withdraw(
    session: &dyn RelaySession,
    diagnostics: &Diagnostics,
    attribution: &BTreeMap<SubscriptionId, Filter>,
) {
    for id in attribution.keys() {
        let frame = match encode_client(&ClientMessage::close(id.clone())) {
            Ok(frame) => frame,
            Err(error) => {
                diagnostics.failed(
                    session.key().clone(),
                    session.generation(),
                    error.to_string(),
                );
                continue;
            }
        };
        match session.send(frame).await {
            HandoffOutcome::HandedOff => {
                diagnostics.withdrawn(session.key().clone(), session.generation(), id.clone());
            }
            HandoffOutcome::NotHandedOff { reason } | HandoffOutcome::Ambiguous { reason } => {
                diagnostics.failed(session.key().clone(), session.generation(), reason);
            }
        }
    }
    if let Err(error) = session.close().await {
        diagnostics.failed(
            session.key().clone(),
            session.generation(),
            error.to_string(),
        );
    }
}

/// Exact text for the limits one relay currently declares.
fn describe(limitation: &RelayLimitation) -> String {
    let mut declared = Vec::new();
    let mut record = |name: &str, value: Option<usize>| {
        if let Some(value) = value {
            declared.push(format!("{name}={value}"));
        }
    };
    record("max_subscriptions", limitation.max_subscriptions);
    record("max_filters", limitation.max_filters);
    record("max_message_length", limitation.max_message_length);
    record("max_subid_length", limitation.max_subid_length);
    record("max_limit", limitation.max_limit);
    record("max_content_length", limitation.max_content_length);
    record("max_event_tags", limitation.max_event_tags);
    if let Some(difficulty) = limitation.min_pow_difficulty {
        declared.push(format!("min_pow_difficulty={difficulty}"));
    }
    if let Some(required) = limitation.auth_required {
        declared.push(format!("auth_required={required}"));
    }
    if let Some(restricted) = limitation.restricted_writes {
        declared.push(format!("restricted_writes={restricted}"));
    }
    if declared.is_empty() {
        "declares no limits".to_owned()
    } else {
        declared.join(" ")
    }
}
