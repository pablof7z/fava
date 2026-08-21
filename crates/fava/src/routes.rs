use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use fava_observe::{Observation, ObserveError};
use fava_query::Query;
use fava_routing::{RoutePlan, RouteRequest, RouterSession};
use fava_state::RelaySessionKey;
use fava_subscriptions::SubscriptionPlanner;
use fava_transport::Transport;
use tokio::sync::watch;

use super::Fava;
use super::relay::OpenedRelay;

pub(super) async fn open(fava: &Fava, query: Query) -> Result<Observation, ObserveError> {
    let planner = fava.subscription_planner.as_ref().ok_or_else(|| {
        ObserveError::Relay("live queries require a subscription planner".to_owned())
    })?;
    let transport = fava
        .transport
        .as_ref()
        .ok_or_else(|| ObserveError::Relay("live queries require a transport".to_owned()))?;
    let request = RouteRequest::Read(query.clone());
    let routes = fava_routing::open(&fava.routers, &request)
        .map_err(|error| ObserveError::Relay(error.to_string()))?;
    for router in &fava.routers {
        fava.diagnostics.router_opened(router.name().to_owned());
    }
    let initial = RoutePlan::from_contribution(1, &routes.current())
        .map_err(|error| ObserveError::Relay(error.to_string()))?;
    record_plan(fava, &initial);

    let mut observation = fava.observer.open(query.clone())?;
    let providers = Providers {
        transport: Arc::clone(transport),
        planner: Arc::clone(planner),
        cache: Arc::clone(&fava.event_cache),
        diagnostics: Arc::clone(&fava.diagnostics),
        next_subscription: Arc::clone(&fava.next_subscription),
        authentication: fava.authentication.clone(),
    };
    let mut active = BTreeMap::new();
    add_relays(
        &query,
        &providers,
        initial.destinations.keys().cloned().collect(),
        &mut active,
        initial.revision,
    )
    .await;

    let (cancel, cancel_rx) = watch::channel(false);
    observation.attach_cancellation(cancel);
    tokio::spawn(run(
        query,
        providers,
        routes,
        active,
        cancel_rx,
        initial.revision,
    ));
    Ok(observation)
}

struct Providers {
    transport: Arc<dyn Transport>,
    planner: Arc<dyn SubscriptionPlanner>,
    cache: Arc<dyn fava_event_cache::EventCache>,
    diagnostics: Arc<fava_diagnostics::Diagnostics>,
    next_subscription: Arc<std::sync::atomic::AtomicU64>,
    authentication: Option<Arc<fava_auth::Authentication>>,
}

async fn run(
    query: Query,
    providers: Providers,
    mut routes: Box<dyn RouterSession>,
    mut active: BTreeMap<RelaySessionKey, watch::Sender<bool>>,
    mut cancel: watch::Receiver<bool>,
    mut revision: u64,
) {
    loop {
        let changed = tokio::select! {
            biased;
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow_and_update() {
                    break;
                }
                continue;
            }
            changed = routes.next_change() => changed,
        };
        let contribution = match changed {
            Ok(contribution) => contribution,
            Err(error) => {
                providers
                    .diagnostics
                    .route_shortfall(revision, error.to_string());
                break;
            }
        };
        revision = revision.saturating_add(1);
        let plan = match RoutePlan::from_contribution(revision, &contribution) {
            Ok(plan) => plan,
            Err(error) => {
                providers
                    .diagnostics
                    .route_shortfall(revision, error.to_string());
                break;
            }
        };
        providers
            .diagnostics
            .route(revision, plan.destinations.keys().cloned().collect());
        let desired: BTreeSet<_> = plan.destinations.keys().cloned().collect();
        let removed: Vec<_> = active
            .keys()
            .filter(|relay| !desired.contains(*relay))
            .cloned()
            .collect();
        for relay in removed {
            if let Some(cancel) = active.remove(&relay) {
                cancel.send_replace(true);
            }
        }
        let added = desired
            .into_iter()
            .filter(|relay| !active.contains_key(relay))
            .collect();
        add_relays(&query, &providers, added, &mut active, revision).await;
    }
    routes.close();
    for cancel in active.into_values() {
        cancel.send_replace(true);
    }
}

async fn add_relays(
    query: &Query,
    providers: &Providers,
    relays: Vec<RelaySessionKey>,
    active: &mut BTreeMap<RelaySessionKey, watch::Sender<bool>>,
    revision: u64,
) {
    for relay in relays {
        match OpenedRelay::open(
            relay.clone(),
            query.clone(),
            Arc::clone(&providers.transport),
            Arc::clone(&providers.planner),
            Arc::clone(&providers.cache),
            Arc::clone(&providers.diagnostics),
            Arc::clone(&providers.next_subscription),
            providers.authentication.clone(),
        )
        .await
        {
            Ok(opened) => {
                let (cancel, cancel_rx) = watch::channel(false);
                active.insert(relay, cancel);
                tokio::spawn(opened.run(cancel_rx));
            }
            Err(error) => providers
                .diagnostics
                .route_shortfall(revision, format!("{relay:?}: {error}")),
        }
    }
}

fn record_plan(fava: &Fava, plan: &RoutePlan) {
    fava.diagnostics
        .route(plan.revision, plan.destinations.keys().cloned().collect());
    for shortfall in &plan.shortfalls {
        fava.diagnostics
            .route_shortfall(plan.revision, shortfall.clone());
    }
}
