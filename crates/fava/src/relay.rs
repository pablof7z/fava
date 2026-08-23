use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_ingest::{RelayIngestError, admit_subscription_event};
use fava_query::Query;
use fava_state::{RelaySessionKey, Timestamp};
use fava_subscriptions::{
    InstalledSubscriptions, ObservationId, PlanRevision, QueryBranchId, RelayReadConstraints,
    SubscriptionPlanner, demand_for_query, validate_plan,
};
use fava_transport::{HandoffOutcome, RelaySession, Transport};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId, decode_relay, encode_client};
use nostr::filter::Filter;
use tokio::sync::watch;

pub(super) struct OpenedRelay {
    session_key: RelaySessionKey,
    query: Query,
    transport: Arc<dyn Transport>,
    planner: Arc<dyn SubscriptionPlanner>,
    cache: Arc<dyn EventCache>,
    diagnostics: Arc<Diagnostics>,
    next_subscription: Arc<AtomicU64>,
    session: Arc<dyn RelaySession>,
    attribution: BTreeMap<SubscriptionId, Filter>,
}

impl OpenedRelay {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn open(
        session_key: RelaySessionKey,
        query: Query,
        transport: Arc<dyn Transport>,
        planner: Arc<dyn SubscriptionPlanner>,
        cache: Arc<dyn EventCache>,
        diagnostics: Arc<Diagnostics>,
        next_subscription: Arc<AtomicU64>,
    ) -> Result<Self, String> {
        let (session, attribution) = establish(
            &session_key,
            &query,
            transport.as_ref(),
            planner.as_ref(),
            diagnostics.as_ref(),
            next_subscription.as_ref(),
        )
        .await?;
        Ok(Self {
            session_key,
            query,
            transport,
            planner,
            cache,
            diagnostics,
            next_subscription,
            session,
            attribution,
        })
    }

    pub(super) async fn abort(self) {
        withdraw(
            self.session.as_ref(),
            self.diagnostics.as_ref(),
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
                            self.diagnostics.as_ref(),
                            &self.attribution,
                        ).await;
                        return;
                    }
                }
                inbound = self.session.next_message() => {
                    match inbound {
                        Ok(frame) => self.handle_frame(&frame),
                        Err(error) => {
                            self.diagnostics.failed(
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

    fn handle_frame(&self, frame: &str) {
        let generation = self.session.generation();
        let message = match decode_relay(frame) {
            Ok(message) => message,
            Err(error) => {
                self.diagnostics.failed(
                    self.session_key.clone(),
                    generation,
                    format!("invalid relay message: {error}"),
                );
                return;
            }
        };
        handle_message(
            self.session.as_ref(),
            self.cache.as_ref(),
            self.diagnostics.as_ref(),
            &self.attribution,
            message,
        );
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
            let reconnect = establish(
                &self.session_key,
                &self.query,
                self.transport.as_ref(),
                self.planner.as_ref(),
                self.diagnostics.as_ref(),
                self.next_subscription.as_ref(),
            );
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
                Ok((session, attribution)) => {
                    self.session = session;
                    self.attribution = attribution;
                    return true;
                }
                Err(error) => self.diagnostics.failed(
                    self.session_key.clone(),
                    self.session.generation(),
                    format!("reconnect refused: {error}"),
                ),
            }
        }
    }
}

async fn establish(
    session_key: &RelaySessionKey,
    query: &Query,
    transport: &dyn Transport,
    planner: &dyn SubscriptionPlanner,
    diagnostics: &Diagnostics,
    next_subscription: &AtomicU64,
) -> Result<(Arc<dyn RelaySession>, BTreeMap<SubscriptionId, Filter>), String> {
    let owner = allocate_observation(next_subscription)?;
    let demand = [demand_for_query(owner, QueryBranchId::ROOT, query)];
    let constraints = RelayReadConstraints::unknown();
    let installed = InstalledSubscriptions::empty();
    let plan = planner
        .plan(
            session_key,
            &demand,
            &constraints,
            &installed,
            PlanRevision(1),
        )
        .map_err(|error| error.to_string())?;
    validate_plan(session_key, &demand, &constraints, &installed, &plan)
        .map_err(|error| error.to_string())?;
    let session = transport
        .open_session(session_key.clone())
        .await
        .map_err(|error| error.to_string())?;
    if session.key() != session_key {
        let _ = session.close().await;
        return Err("transport returned the wrong relay session identity".to_owned());
    }
    let generation = session.generation();
    diagnostics.session_opened(session_key.clone(), generation);
    for planned in &plan.open {
        let message = ClientMessage::Req {
            subscription_id: std::borrow::Cow::Owned(planned.id.clone()),
            filters: planned
                .filters
                .iter()
                .map(|filter| std::borrow::Cow::Owned(filter.clone()))
                .collect(),
        };
        let frame = encode_client(&message).map_err(|error| error.to_string())?;
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
    let mut attribution = BTreeMap::new();
    for id in plan.attribution.ids() {
        diagnostics.subscription_opened(session_key.clone(), generation, id.clone());
        if let Some(entry) = plan.attribution.get(id)
            && let Some(filter) = entry.filters.first()
        {
            attribution.insert(id.clone(), filter.clone());
        }
    }
    Ok((session, attribution))
}

fn allocate_observation(next: &AtomicU64) -> Result<ObservationId, String> {
    let sequence = next
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(1)
        })
        .map_err(|_| "observation identity exhausted".to_owned())?
        + 1;
    std::num::NonZeroU64::new(sequence)
        .map(ObservationId::new)
        .ok_or_else(|| "observation identity exhausted".to_owned())
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
            if let Err(error) = admit_subscription_event(
                cache,
                session.key(),
                attribution,
                &id,
                event.into_owned(),
                Timestamp::now(),
            ) {
                let reason = match &error {
                    RelayIngestError::WrongSubscription => {
                        format!("unattributed EVENT for {id}")
                    }
                    other => other.to_string(),
                };
                diagnostics.failed(key, generation, reason);
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
        RelayMessage::Auth { .. } => diagnostics.authentication_required(key, generation),
        RelayMessage::Notice(message) => {
            diagnostics.failed(key, generation, format!("relay NOTICE: {message}"));
        }
        RelayMessage::Ok { .. }
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
