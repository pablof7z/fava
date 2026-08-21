use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use crate::{
    CoverageState, RouteContribution, RoutePlan, RouteRequest, Router, RouterError, RouterSession,
    merge_coverage,
};

const MAX_ROUTERS: usize = 32;
const MAX_DESTINATIONS: usize = 256;
const MAX_TARGETS: usize = 256;
const MAX_COVERAGE: usize = 256;
const MAX_COVERED_SESSIONS: usize = 256;
const MAX_SHORTFALLS: usize = 256;
const MAX_TEXT_BYTES: usize = 4_096;

/// Evaluate the ordered router chain without opening router sessions.
///
/// # Errors
///
/// Returns [`RouterError`] for invalid configuration, refusal, or bounded output.
pub fn preview(
    routers: &[Arc<dyn Router>],
    request: &RouteRequest,
) -> Result<RoutePlan, RouterError> {
    validate_names(routers)?;
    let mut contributions = Vec::with_capacity(routers.len());
    let targets = request.targets();
    for router in routers {
        let upstream =
            RoutePlan::from_contribution(1, &complete_targets(combine(&contributions), &targets))?;
        contributions.push(attribute(
            router.preview(request, &upstream)?,
            router.name(),
        )?);
    }
    RoutePlan::from_contribution(1, &complete_targets(combine(&contributions), &targets))
}

/// Open the ordered router chain as one complete-contribution sequence.
///
/// # Errors
///
/// Returns [`RouterError`] for invalid configuration, refusal, or bounded output.
pub fn open(
    routers: &[Arc<dyn Router>],
    request: &RouteRequest,
) -> Result<Box<dyn RouterSession>, RouterError> {
    validate_names(routers)?;
    let targets = request.targets();
    let mut sessions = Vec::with_capacity(routers.len());
    let mut contributions = Vec::with_capacity(routers.len());
    let mut upstream = Vec::with_capacity(routers.len());
    for router in routers {
        let plan =
            RoutePlan::from_contribution(1, &complete_targets(combine(&contributions), &targets))?;
        let (upstream_tx, upstream_rx) = watch::channel(Arc::new(plan));
        let mut session = match router.open(request.clone(), upstream_rx) {
            Ok(session) => session,
            Err(error) => {
                close_sessions(&mut sessions);
                return Err(error);
            }
        };
        let contribution = match attribute(session.current(), router.name()) {
            Ok(contribution) => contribution,
            Err(error) => {
                session.close();
                close_sessions(&mut sessions);
                return Err(error);
            }
        };
        contributions.push(contribution);
        upstream.push(upstream_tx);
        sessions.push((router.name().to_owned(), session));
    }
    let initial = Arc::new(complete_targets(combine(&contributions), &targets));
    let (latest_tx, latest) = watch::channel(initial);
    let (cancel, cancel_rx) = watch::channel(false);
    let (updates_tx, updates_rx) = mpsc::channel(routers.len().max(1));
    for (index, (name, session)) in sessions.into_iter().enumerate() {
        tokio::spawn(monitor_router(
            index,
            name,
            session,
            updates_tx.clone(),
            cancel_rx.clone(),
        ));
    }
    drop(updates_tx);
    tokio::spawn(compose_updates(
        contributions,
        upstream,
        updates_rx,
        latest_tx,
        cancel_rx,
        targets,
    ));
    Ok(Box::new(OpenedChain {
        latest,
        cancel,
        closed: false,
    }))
}

struct OpenedChain {
    latest: watch::Receiver<Arc<RouteContribution>>,
    cancel: watch::Sender<bool>,
    closed: bool,
}

impl RouterSession for OpenedChain {
    fn current(&self) -> RouteContribution {
        self.latest.borrow().as_ref().clone()
    }

    fn next_change(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<RouteContribution, RouterError>> + Send + '_>> {
        Box::pin(async move {
            if self.closed || self.latest.changed().await.is_err() {
                return Err(RouterError::Closed);
            }
            Ok(self.latest.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.cancel.send_replace(true);
        }
    }
}

impl Drop for OpenedChain {
    fn drop(&mut self) {
        self.close();
    }
}

struct RouterUpdate {
    index: usize,
    name: String,
    contribution: Result<RouteContribution, RouterError>,
}

async fn monitor_router(
    index: usize,
    name: String,
    mut session: Box<dyn RouterSession>,
    updates: mpsc::Sender<RouterUpdate>,
    mut cancel: watch::Receiver<bool>,
) {
    loop {
        let contribution = tokio::select! {
            biased;
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow_and_update() {
                    break;
                }
                continue;
            }
            contribution = session.next_change() => contribution,
        };
        let closed = contribution.is_err();
        if updates
            .send(RouterUpdate {
                index,
                name: name.clone(),
                contribution,
            })
            .await
            .is_err()
            || closed
        {
            break;
        }
    }
    session.close();
}

async fn compose_updates(
    mut contributions: Vec<RouteContribution>,
    upstream: Vec<watch::Sender<Arc<RoutePlan>>>,
    mut updates: mpsc::Receiver<RouterUpdate>,
    latest: watch::Sender<Arc<RouteContribution>>,
    mut cancel: watch::Receiver<bool>,
    targets: BTreeSet<crate::RouteTarget>,
) {
    let mut revision = 1_u64;
    loop {
        let update = tokio::select! {
            biased;
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow_and_update() {
                    break;
                }
                continue;
            }
            update = updates.recv() => update,
        };
        let Some(update) = update else {
            break;
        };
        contributions[update.index] = live_contribution(update.contribution, &update.name);
        revision = revision.saturating_add(1);
        for index in (update.index + 1)..upstream.len() {
            let plan = RoutePlan::from_contribution(
                revision,
                &complete_targets(combine(&contributions[..index]), &targets),
            )
            .expect("validated router contributions remain bounded when combined");
            upstream[index].send_replace(Arc::new(plan));
        }
        latest.send_replace(Arc::new(complete_targets(
            combine(&contributions),
            &targets,
        )));
    }
}

fn live_contribution(
    contribution: Result<RouteContribution, RouterError>,
    router: &str,
) -> RouteContribution {
    match contribution.and_then(|contribution| attribute(contribution, router)) {
        Ok(contribution) => contribution,
        Err(error) => RouteContribution {
            shortfalls: vec![bounded_error(router, &error)],
            ..RouteContribution::default()
        },
    }
}

fn bounded_error(router: &str, error: &RouterError) -> String {
    let rendered = format!("{router}: {error}");
    if rendered.len() <= MAX_TEXT_BYTES {
        rendered
    } else {
        format!("{router}: router error text exceeds {MAX_TEXT_BYTES}-byte bound")
    }
}

fn attribute(
    mut contribution: RouteContribution,
    router: &str,
) -> Result<RouteContribution, RouterError> {
    validate_router_contribution(&contribution)?;
    for destination in &mut contribution.destinations {
        destination.set_router(router);
    }
    Ok(contribution)
}

fn combine(contributions: &[RouteContribution]) -> RouteContribution {
    let mut combined = RouteContribution::default();
    for contribution in contributions {
        combined
            .destinations
            .extend(contribution.destinations.iter().cloned());
        for (target, state) in &contribution.coverage {
            if matches!(state, CoverageState::Unresolved) {
                combined.unresolved.insert(target.clone());
            }
            merge_coverage(
                combined
                    .coverage
                    .entry(target.clone())
                    .or_insert(CoverageState::SettledAbsent),
                state,
            );
        }
        combined
            .unresolved
            .extend(contribution.unresolved.iter().cloned());
        combined.shortfalls.extend(contribution.shortfalls.clone());
    }
    combined
}

fn complete_targets(
    mut contribution: RouteContribution,
    targets: &BTreeSet<crate::RouteTarget>,
) -> RouteContribution {
    for target in targets {
        contribution
            .coverage
            .entry(target.clone())
            .or_insert_with(|| {
                if contribution.unresolved.contains(target) {
                    CoverageState::Unresolved
                } else {
                    CoverageState::SettledAbsent
                }
            });
    }
    contribution
}

pub(crate) fn validate_router_contribution(
    contribution: &RouteContribution,
) -> Result<(), RouterError> {
    validate_contribution(contribution, 1)
}

pub(crate) fn validate_combined(contribution: &RouteContribution) -> Result<(), RouterError> {
    validate_contribution(contribution, MAX_ROUTERS)
}

fn validate_contribution(
    contribution: &RouteContribution,
    factor: usize,
) -> Result<(), RouterError> {
    bounded(
        "route destinations",
        contribution.destinations.len(),
        MAX_DESTINATIONS * factor,
    )?;
    bounded(
        "route coverage targets",
        contribution.coverage.len(),
        MAX_COVERAGE * factor,
    )?;
    bounded(
        "unresolved route targets",
        contribution.unresolved.len(),
        MAX_TARGETS * factor,
    )?;
    bounded(
        "route shortfalls",
        contribution.shortfalls.len(),
        MAX_SHORTFALLS * factor,
    )?;
    for destination in &contribution.destinations {
        bounded(
            "destination targets",
            destination.targets.len(),
            MAX_TARGETS,
        )?;
        bounded_text("route reason", &destination.reason)?;
    }
    for state in contribution.coverage.values() {
        if let CoverageState::Covered(sessions) = state {
            bounded(
                "covered relay sessions",
                sessions.len(),
                MAX_COVERED_SESSIONS,
            )?;
        }
    }
    for shortfall in &contribution.shortfalls {
        bounded_text("route shortfall", shortfall)?;
    }
    Ok(())
}

fn bounded(label: &str, actual: usize, maximum: usize) -> Result<(), RouterError> {
    if actual > maximum {
        Err(RouterError::Refused(format!(
            "{label} exceed bound: {actual} > {maximum}"
        )))
    } else {
        Ok(())
    }
}

fn bounded_text(label: &str, value: &str) -> Result<(), RouterError> {
    bounded(label, value.len(), MAX_TEXT_BYTES)
}

fn validate_names(routers: &[Arc<dyn Router>]) -> Result<(), RouterError> {
    bounded("configured routers", routers.len(), MAX_ROUTERS)?;
    let mut names = BTreeSet::new();
    for router in routers {
        if router.name().is_empty() {
            return Err(RouterError::Refused("router name is empty".to_owned()));
        }
        bounded_text("router name", router.name())?;
        if !names.insert(router.name()) {
            return Err(RouterError::Refused(format!(
                "duplicate router name: {}",
                router.name()
            )));
        }
    }
    Ok(())
}

fn close_sessions(sessions: &mut Vec<(String, Box<dyn RouterSession>)>) {
    for (_, session) in sessions {
        session.close();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};

    use super::*;
    use crate::{RouteDestination, RouteTarget};

    #[test]
    fn refuses_router_contribution_over_destination_bound() {
        let destinations = (0..=MAX_DESTINATIONS)
            .map(|index| {
                RouteDestination::new(
                    RelaySessionKey::new(
                        RelayUrl::parse(&format!("wss://relay-{index}.example")).unwrap(),
                        RelayAccess::public(),
                    ),
                    BTreeSet::from([RouteTarget::WholeRequest]),
                    "test route",
                )
            })
            .collect();
        let error = validate_router_contribution(&RouteContribution {
            destinations,
            ..RouteContribution::default()
        })
        .unwrap_err();

        assert_eq!(
            error,
            RouterError::Refused("route destinations exceed bound: 257 > 256".to_owned())
        );
    }

    #[test]
    fn refuses_router_count_over_bound_with_exact_numbers() {
        assert_eq!(
            bounded("configured routers", MAX_ROUTERS + 1, MAX_ROUTERS),
            Err(RouterError::Refused(
                "configured routers exceed bound: 33 > 32".to_owned()
            ))
        );
    }

    #[test]
    fn routing_core_does_not_name_concrete_router_crates_or_types() {
        let cargo = include_str!("../Cargo.toml");
        let public_source = include_str!("lib.rs");
        for forbidden in [
            "fava-router-outbox",
            "fava-router-hints",
            "fava-router-app-relays",
            "fava-router-fallback-relays",
            "OutboxRouter",
            "HintRouter",
            "AppRelayRouter",
            "FallbackRelayRouter",
        ] {
            assert!(!cargo.contains(forbidden));
            assert!(!public_source.contains(forbidden));
        }
    }
}
