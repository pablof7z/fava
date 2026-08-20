use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_ingest::admit_subscription_event;
use fava_observe::{Observation, ObserveError};
use fava_query::{Query, QueryAcquisition};
use fava_state::{RelaySessionKey, Timestamp};
use fava_subscriptions::{SubscriptionPlan, demand_for_query};
use fava_transport::{HandoffOutcome, RelaySession};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId, decode_relay, encode_client};
use nostr::filter::Filter;
use tokio::sync::watch;

use super::Fava;

pub(super) async fn open(fava: &Fava, query: Query) -> Result<Observation, ObserveError> {
    let relays = match query.source().acquisition() {
        QueryAcquisition::Explicit(relays) => relays,
        QueryAcquisition::Automatic => {
            return Err(ObserveError::Relay(
                "automatic routing is unavailable in this profile".to_owned(),
            ));
        }
    };
    if relays.len() != 1 {
        return Err(ObserveError::Relay(
            "this profile requires exactly one explicit relay".to_owned(),
        ));
    }
    let planner = fava.subscription_planner.as_ref().ok_or_else(|| {
        ObserveError::Relay("live queries require a subscription planner".to_owned())
    })?;
    let transport = fava
        .transport
        .as_ref()
        .ok_or_else(|| ObserveError::Relay("live queries require a transport".to_owned()))?;
    let sequence = fava
        .next_subscription
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(1)
        })
        .map_err(|_| ObserveError::Relay("subscription identity exhausted".to_owned()))?
        + 1;
    let subscription = SubscriptionId::new(format!("fava-{sequence}"));
    let relay = relays.iter().next().expect("non-empty checked").clone();
    let session_key = RelaySessionKey::new(relay, query.access().clone());
    let demand = demand_for_query(subscription, &query);
    let plan = planner
        .plan(&session_key, &[demand])
        .map_err(|error| ObserveError::Relay(error.to_string()))?;
    validate_plan(&session_key, &plan)?;

    let mut observation = fava.observer.open(query)?;
    let session = match transport.open_session(session_key.clone()).await {
        Ok(session) => session,
        Err(error) => {
            observation.close();
            return Err(ObserveError::Relay(error.to_string()));
        }
    };
    if session.key() != &session_key {
        observation.close();
        let _ = session.close().await;
        return Err(ObserveError::Relay(
            "transport returned the wrong relay session identity".to_owned(),
        ));
    }
    let generation = session.generation();
    fava.diagnostics
        .session_opened(session_key.clone(), generation);
    for message in &plan.messages {
        let frame =
            encode_client(message).map_err(|error| ObserveError::Relay(error.to_string()))?;
        match session.send(frame).await {
            HandoffOutcome::HandedOff => {}
            HandoffOutcome::NotHandedOff { reason } => {
                observation.close();
                let _ = session.close().await;
                return Err(ObserveError::Relay(format!(
                    "subscription was not handed off: {reason}"
                )));
            }
            HandoffOutcome::Ambiguous { reason } => {
                observation.close();
                let _ = session.close().await;
                return Err(ObserveError::Relay(format!(
                    "subscription handoff is ambiguous: {reason}"
                )));
            }
        }
    }
    for id in plan.attribution.keys() {
        fava.diagnostics
            .subscription_opened(session_key.clone(), generation, id.clone());
    }

    let (cancel, cancel_rx) = watch::channel(false);
    observation.attach_cancellation(cancel);
    tokio::spawn(relay_loop(
        session,
        Arc::clone(&fava.event_cache),
        Arc::clone(&fava.diagnostics),
        plan.attribution,
        cancel_rx,
    ));
    Ok(observation)
}

fn validate_plan(
    expected_relay: &RelaySessionKey,
    plan: &SubscriptionPlan,
) -> Result<(), ObserveError> {
    if &plan.relay != expected_relay || plan.attribution.is_empty() || plan.messages.is_empty() {
        return Err(ObserveError::Relay(
            "subscription planner returned incomplete or mis-scoped work".to_owned(),
        ));
    }
    for message in &plan.messages {
        let ClientMessage::Req {
            subscription_id,
            filters,
        } = message
        else {
            return Err(ObserveError::Relay(
                "subscription planner returned a non-REQ message".to_owned(),
            ));
        };
        if filters.len() != 1
            || plan.attribution.get(subscription_id.as_ref()) != Some(filters[0].as_ref())
        {
            return Err(ObserveError::Relay(
                "subscription planner attribution does not match its REQ".to_owned(),
            ));
        }
    }
    Ok(())
}

async fn relay_loop(
    session: Arc<dyn RelaySession>,
    cache: Arc<dyn EventCache>,
    diagnostics: Arc<Diagnostics>,
    attribution: BTreeMap<SubscriptionId, Filter>,
    mut cancel: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            biased;
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow_and_update() {
                    withdraw(session.as_ref(), diagnostics.as_ref(), &attribution).await;
                    return;
                }
            }
            inbound = session.next_message() => {
                let frame = match inbound {
                    Ok(frame) => frame,
                    Err(error) => {
                        diagnostics.failed(
                            session.key().clone(),
                            session.generation(),
                            error.to_string(),
                        );
                        return;
                    }
                };
                let message = match decode_relay(&frame) {
                    Ok(message) => message,
                    Err(error) => {
                        diagnostics.failed(
                            session.key().clone(),
                            session.generation(),
                            format!("invalid relay message: {error}"),
                        );
                        continue;
                    }
                };
                handle_message(
                    session.as_ref(),
                    cache.as_ref(),
                    diagnostics.as_ref(),
                    &attribution,
                    message,
                );
            }
        }
    }
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
