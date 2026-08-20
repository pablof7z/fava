use std::sync::Arc;

use fava_observe::{Observation, ObserveError};
use fava_query::{Query, QueryAcquisition};
use fava_state::RelaySessionKey;
use tokio::sync::watch;

use super::Fava;
use super::relay::OpenedRelay;

pub(super) async fn open(fava: &Fava, query: Query) -> Result<Observation, ObserveError> {
    match query.source().acquisition() {
        QueryAcquisition::Explicit(_) => open_explicit(fava, query).await,
        QueryAcquisition::Automatic => super::routes::open(fava, query).await,
    }
}

async fn open_explicit(fava: &Fava, query: Query) -> Result<Observation, ObserveError> {
    let QueryAcquisition::Explicit(relays) = query.source().acquisition() else {
        unreachable!("caller selected explicit acquisition");
    };
    let planner = fava.subscription_planner.as_ref().ok_or_else(|| {
        ObserveError::Relay("live queries require a subscription planner".to_owned())
    })?;
    let transport = fava
        .transport
        .as_ref()
        .ok_or_else(|| ObserveError::Relay("live queries require a transport".to_owned()))?;

    let mut observation = fava.observer.open(query.clone())?;
    let mut opened = Vec::with_capacity(relays.len());
    for relay in relays {
        let session_key = RelaySessionKey::new(relay.clone(), query.access().clone());
        match OpenedRelay::open(
            session_key,
            query.clone(),
            Arc::clone(transport),
            Arc::clone(planner),
            Arc::clone(&fava.event_cache),
            Arc::clone(&fava.diagnostics),
            Arc::clone(&fava.next_subscription),
        )
        .await
        {
            Ok(relay) => opened.push(relay),
            Err(error) => {
                observation.close();
                for relay in opened {
                    relay.abort().await;
                }
                return Err(ObserveError::Relay(error));
            }
        }
    }

    for relay in opened {
        let (cancel, cancel_rx) = watch::channel(false);
        observation.attach_cancellation(cancel);
        tokio::spawn(relay.run(cancel_rx));
    }
    Ok(observation)
}
