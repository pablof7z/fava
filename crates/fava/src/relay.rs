use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_ingest::{RelayIngestError, admit_subscription_event};
use fava_query::Query;
use fava_state::{RelaySessionKey, Timestamp};
use fava_subscriptions::{SubscriptionPlan, SubscriptionPlanner, demand_for_query};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, RelayInbound, RelayMessageStream,
    RelaySession, RelaySessionLease, Transport, TransportBounds, TransportDeadlines,
};
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
    lease: RelaySessionLease,
    attribution: BTreeMap<SubscriptionId, Filter>,
}

/// Transport request this call site hands the transport. Phase 07.6 replaces
/// this whole file; the durations state the previous behaviour explicitly.
fn open_request(session_key: &RelaySessionKey) -> OpenRelaySession {
    let frames = |count: usize| NonZeroUsize::new(count).expect("constant is non-zero");
    OpenRelaySession {
        key: session_key.clone(),
        deadlines: TransportDeadlines {
            establish: Duration::from_secs(10),
            write: Duration::from_secs(10),
            idle: Duration::from_secs(120),
            close: Duration::from_secs(5),
        },
        bounds: TransportBounds {
            inbound_frames: frames(256),
            outbound_frames: frames(64),
            max_frame_bytes: frames(1_048_576),
        },
        reconnect_attempts: None,
    }
}

fn generation_of(session: &dyn RelaySession) -> u64 {
    session.identity().generation.0
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
        let (lease, attribution) = establish(
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
            lease,
            attribution,
        })
    }

    pub(super) async fn abort(self) {
        withdraw(
            self.lease.session().as_ref(),
            self.diagnostics.as_ref(),
            &self.attribution,
        )
        .await;
    }

    pub(super) async fn run(mut self, mut cancel: watch::Receiver<bool>) {
        let mut stream: Box<dyn RelayMessageStream> = self.lease.session().messages();
        loop {
            let inbound = tokio::select! {
                biased;
                changed = cancel.changed() => {
                    if changed.is_err() || *cancel.borrow_and_update() {
                        withdraw(
                            self.lease.session().as_ref(),
                            self.diagnostics.as_ref(),
                            &self.attribution,
                        ).await;
                        return;
                    }
                    continue;
                }
                inbound = stream.next_inbound() => inbound,
            };
            let failure = match inbound {
                Ok(RelayInbound::Frame { frame, .. }) => {
                    self.handle_frame(&String::from_utf8_lossy(&frame));
                    continue;
                }
                Ok(RelayInbound::Reconnected { .. }) => continue,
                Ok(RelayInbound::Lost { dropped, .. }) => {
                    format!("{dropped} inbound relay items were dropped")
                }
                Ok(RelayInbound::Disconnected { reason, .. }) => format!("{reason:?}"),
                Ok(RelayInbound::ReconnectExhausted { reason, .. }) => {
                    format!("reconnect exhausted: {reason:?}")
                }
                Err(error) => error.to_string(),
            };
            self.diagnostics.failed(
                self.session_key.clone(),
                generation_of(self.lease.session().as_ref()),
                failure,
            );
            if !self.reconnect(&mut cancel).await {
                return;
            }
            stream = self.lease.session().messages();
        }
    }

    fn handle_frame(&self, frame: &str) {
        let generation = generation_of(self.lease.session().as_ref());
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
            self.lease.session().as_ref(),
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
                Ok((lease, attribution)) => {
                    self.lease = lease;
                    self.attribution = attribution;
                    return true;
                }
                Err(error) => self.diagnostics.failed(
                    self.session_key.clone(),
                    generation_of(self.lease.session().as_ref()),
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
) -> Result<(RelaySessionLease, BTreeMap<SubscriptionId, Filter>), String> {
    let subscription = allocate_subscription(next_subscription)?;
    let plan = planner
        .plan(session_key, &[demand_for_query(subscription, query)])
        .map_err(|error| error.to_string())?;
    validate_plan(session_key, &plan)?;
    let lease = transport
        .acquire_session(open_request(session_key))
        .await
        .map_err(|error| error.to_string())?;
    let identity = lease.session().identity();
    if identity.key != *session_key {
        let _ = lease.session().close().await;
        return Err("transport returned the wrong relay session identity".to_owned());
    }
    let generation = identity.generation.0;
    diagnostics.session_opened(session_key.clone(), generation);
    for (index, message) in plan.messages.iter().enumerate() {
        let frame = encode_client(message).map_err(|error| error.to_string())?;
        let correlation = HandoffCorrelation(index as u64);
        match lease.session().send(frame.into_bytes(), correlation).await {
            HandoffOutcome::HandedOff { .. } => {}
            HandoffOutcome::NotHandedOff { reason, .. } => {
                let _ = lease.session().close().await;
                return Err(format!("subscription was not handed off: {reason:?}"));
            }
            HandoffOutcome::Ambiguous { reason, .. } => {
                let _ = lease.session().close().await;
                return Err(format!("subscription handoff is ambiguous: {reason:?}"));
            }
        }
    }
    for id in plan.attribution.keys() {
        diagnostics.subscription_opened(session_key.clone(), generation, id.clone());
    }
    Ok((lease, plan.attribution))
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
    let identity = session.identity();
    let key = identity.key.clone();
    let generation = identity.generation.0;
    match message {
        RelayMessage::Event {
            subscription_id,
            event,
        } => {
            let id = subscription_id.into_owned();
            if let Err(error) = admit_subscription_event(
                cache,
                &identity.key,
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
    let identity = session.identity();
    let key = identity.key.clone();
    let generation = identity.generation.0;
    for (index, id) in attribution.keys().enumerate() {
        let frame = match encode_client(&ClientMessage::close(id.clone())) {
            Ok(frame) => frame,
            Err(error) => {
                diagnostics.failed(key.clone(), generation, error.to_string());
                continue;
            }
        };
        let correlation = HandoffCorrelation(index as u64);
        match session.send(frame.into_bytes(), correlation).await {
            HandoffOutcome::HandedOff { .. } => {
                diagnostics.withdrawn(key.clone(), generation, id.clone());
            }
            HandoffOutcome::NotHandedOff { reason, .. } => {
                diagnostics.failed(key.clone(), generation, format!("{reason:?}"));
            }
            HandoffOutcome::Ambiguous { reason, .. } => {
                diagnostics.failed(key.clone(), generation, format!("{reason:?}"));
            }
        }
    }
    if let Err(error) = session.close().await {
        diagnostics.failed(key, generation, error.to_string());
    }
}
