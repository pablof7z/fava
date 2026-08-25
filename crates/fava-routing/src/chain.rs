use std::collections::BTreeSet;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use crate::{
    CoverageState, RouteContribution, RoutePlan, RouteRequest, RouteTarget, Router, RouterError,
    RouterSession, merge_coverage,
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
/// One router's refusal or panic degrades the plan into an attributed shortfall;
/// it never denies the caller the contributions of every other router.
///
/// # Errors
///
/// Returns [`RouterError`] only for invalid chain configuration.
pub fn preview(
    routers: &[Arc<dyn Router>],
    request: &RouteRequest,
) -> Result<RoutePlan, RouterError> {
    validate_names(routers)?;
    let mut composer = Composer::new(routers.len(), request.targets());
    for (index, router) in routers.iter().enumerate() {
        let upstream = composer.upstream_plan(index, 1);
        match preview_router(router.as_ref(), request, &upstream)
            .and_then(|contribution| attribute(contribution, router.name()))
        {
            Ok(contribution) => composer.accept(index, router.name(), contribution),
            Err(error) => composer.record(&bounded_error(router.name(), &error)),
        }
    }
    composer.plan(1)
}

/// Open the ordered router chain as one complete-contribution sequence.
///
/// One router's refusal, panic, or termination is isolated to that router
/// instance: its last coherent contribution is retained and the failure becomes
/// an attributed shortfall in every later plan revision.
///
/// # Errors
///
/// Returns [`RouterError`] only for invalid chain configuration.
pub fn open(
    routers: &[Arc<dyn Router>],
    request: &RouteRequest,
) -> Result<Box<dyn RouterSession>, RouterError> {
    validate_names(routers)?;
    let targets = request.targets();
    let mut composer = Composer::new(routers.len(), targets);
    let mut sessions = Vec::with_capacity(routers.len());
    let mut upstream = Vec::with_capacity(routers.len());
    for (index, router) in routers.iter().enumerate() {
        let plan = composer.upstream_plan(index, 1);
        let (upstream_tx, upstream_rx) = watch::channel(Arc::new(plan));
        upstream.push(upstream_tx);
        match open_router(router.as_ref(), request, upstream_rx) {
            Ok(mut session) => match current_contribution(session.as_mut(), router.name()) {
                Ok(contribution) => {
                    composer.accept(index, router.name(), contribution);
                    sessions.push(Some((router.name().to_owned(), session)));
                }
                Err(error) => {
                    close_session(session.as_mut());
                    composer.record(&bounded_error(router.name(), &error));
                    sessions.push(None);
                }
            },
            Err(error) => {
                composer.record(&bounded_error(router.name(), &error));
                sessions.push(None);
            }
        }
    }
    let (latest_tx, latest) = watch::channel(Arc::new(composer.combined()));
    let (cancel, cancel_rx) = watch::channel(false);
    let (updates_tx, updates_rx) = mpsc::channel(routers.len().max(1));
    for (index, session) in sessions.into_iter().enumerate() {
        if let Some((name, session)) = session {
            tokio::spawn(monitor_router(
                index,
                name,
                session,
                updates_tx.clone(),
                cancel_rx.clone(),
            ));
        }
    }
    drop(updates_tx);
    tokio::spawn(compose_updates(
        composer, upstream, updates_rx, latest_tx, cancel_rx,
    ));
    Ok(Box::new(OpenedChain {
        latest,
        cancel,
        closed: false,
    }))
}

fn preview_router(
    router: &dyn Router,
    request: &RouteRequest,
    upstream: &RoutePlan,
) -> Result<RouteContribution, RouterError> {
    isolate("previewing", || router.preview(request, upstream))
}

fn open_router(
    router: &dyn Router,
    request: &RouteRequest,
    upstream: watch::Receiver<Arc<RoutePlan>>,
) -> Result<Box<dyn RouterSession>, RouterError> {
    isolate("opening", || router.open(request.clone(), upstream))
}

fn current_contribution(
    session: &mut dyn RouterSession,
    router: &str,
) -> Result<RouteContribution, RouterError> {
    isolate("reading its current contribution", || Ok(session.current()))
        .and_then(|contribution| attribute(contribution, router))
}

fn close_session(session: &mut dyn RouterSession) {
    drop(isolate("closing", || {
        session.close();
        Ok(())
    }));
}

/// Run one provider call so a provider panic becomes a scoped typed refusal.
fn isolate<T>(
    action: &str,
    call: impl FnOnce() -> Result<T, RouterError>,
) -> Result<T, RouterError> {
    std::panic::catch_unwind(AssertUnwindSafe(call)).unwrap_or_else(|_| {
        Err(RouterError::Refused(format!(
            "router panicked while {action}"
        )))
    })
}

/// Ordered per-router contributions plus the chain's own bounded shortfalls.
struct Composer {
    contributions: Vec<RouteContribution>,
    /// Whether each configured router has produced a retained contribution.
    ///
    /// A default contribution is indistinguishable from an answer of "I cover
    /// nothing", so absence of coverage is only a routing *fact* once every
    /// configured router has actually answered.
    answered: Vec<bool>,
    shortfalls: Vec<String>,
    discarded: usize,
    targets: BTreeSet<RouteTarget>,
}

impl Composer {
    fn new(routers: usize, targets: BTreeSet<RouteTarget>) -> Self {
        Self {
            contributions: vec![RouteContribution::default(); routers],
            answered: vec![false; routers],
            shortfalls: Vec::new(),
            discarded: 0,
            targets,
        }
    }

    /// Record one chain-owned shortfall, counting exactly what the bound drops.
    fn record(&mut self, shortfall: &str) {
        if self.shortfalls.len() < MAX_SHORTFALLS {
            self.shortfalls.push(shortfall.to_owned());
        } else {
            self.discarded = self.discarded.saturating_add(1);
        }
    }

    /// Replace one router's contribution, keeping the previous one when the
    /// combined result would leave routing bounds.
    fn accept(&mut self, index: usize, router: &str, contribution: RouteContribution) {
        let previous = std::mem::replace(&mut self.contributions[index], contribution);
        let answered = std::mem::replace(&mut self.answered[index], true);
        if let Err(error) = validate_combined(&self.merged()) {
            self.contributions[index] = previous;
            self.answered[index] = answered;
            self.record(&bounded_error(router, &error));
        }
    }

    /// Whether every configured router has produced a retained contribution.
    ///
    /// A router that refuses, panics, or ends *after* a coherent contribution
    /// keeps that contribution and therefore keeps its answer. Only a router
    /// that never produced one is outstanding.
    fn all_answered(&self) -> bool {
        self.answered.iter().all(|answered| *answered)
    }

    fn merged(&self) -> RouteContribution {
        complete_targets(
            combine(&self.contributions),
            &self.targets,
            self.all_answered(),
        )
    }

    /// Complete current chain contribution, always within routing bounds.
    fn combined(&self) -> RouteContribution {
        let mut combined = self.merged();
        combined.shortfalls.extend(self.shortfalls.iter().cloned());
        let mut discarded = self.discarded;
        let maximum = MAX_SHORTFALLS * MAX_ROUTERS;
        if combined.shortfalls.len() >= maximum {
            discarded = discarded.saturating_add(combined.shortfalls.len() - maximum + 1);
            combined.shortfalls.truncate(maximum - 1);
        }
        if discarded > 0 {
            combined.shortfalls.push(format!(
                "chain: {discarded} further route shortfalls discarded beyond the {maximum}-entry bound"
            ));
        }
        combined
    }

    /// Plan visible to the router at `index`, built from earlier routers only.
    fn upstream_plan(&self, index: usize, revision: u64) -> RoutePlan {
        // Routers from `index` onward have not been asked yet, so an upstream
        // view can never report a target as settled absent.
        let answered = index >= self.contributions.len() && self.all_answered();
        let upstream = complete_targets(
            combine(&self.contributions[..index]),
            &self.targets,
            answered,
        );
        RoutePlan::from_contribution(revision, &upstream).unwrap_or_default()
    }

    fn plan(&self, revision: u64) -> Result<RoutePlan, RouterError> {
        RoutePlan::from_contribution(revision, &self.combined())
    }
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
        let ended = contribution.is_err();
        if updates
            .send(RouterUpdate {
                index,
                name: name.clone(),
                contribution,
            })
            .await
            .is_err()
            || ended
        {
            break;
        }
    }
    close_session(session.as_mut());
}

async fn compose_updates(
    mut composer: Composer,
    upstream: Vec<watch::Sender<Arc<RoutePlan>>>,
    mut updates: mpsc::Receiver<RouterUpdate>,
    latest: watch::Sender<Arc<RouteContribution>>,
    mut cancel: watch::Receiver<bool>,
) {
    let mut revision = 1_u64;
    loop {
        let update = tokio::select! {
            biased;
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow_and_update() {
                    return;
                }
                continue;
            }
            update = updates.recv() => update,
        };
        let Some(update) = update else {
            break;
        };
        match update.contribution {
            Ok(contribution) => match attribute(contribution, &update.name) {
                // A later contribution replaces only its own router's demand.
                Ok(contribution) => composer.accept(update.index, &update.name, contribution),
                Err(error) => composer.record(&bounded_error(&update.name, &error)),
            },
            // A router that refuses or ends keeps its last coherent contribution:
            // unchanged destinations stay running and the loss becomes a fact.
            Err(error) => composer.record(&bounded_error(&update.name, &error)),
        }
        revision = revision.saturating_add(1);
        for (index, sender) in upstream.iter().enumerate().skip(update.index + 1) {
            sender.send_replace(Arc::new(composer.upstream_plan(index, revision)));
        }
        latest.send_replace(Arc::new(composer.combined()));
    }
    // Every router instance has ended. The last composed plan stays current and
    // its relay demand stays owned until the application cancels this chain.
    while cancel.changed().await.is_ok() {
        if *cancel.borrow_and_update() {
            return;
        }
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
    for shortfall in &mut contribution.shortfalls {
        let attributed = format!("{router}: {shortfall}");
        *shortfall = if attributed.len() <= MAX_TEXT_BYTES {
            attributed
        } else {
            format!("{router}: route shortfall text exceeds {MAX_TEXT_BYTES}-byte bound")
        };
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

/// Fill every request target no contribution mentioned.
///
/// Settled absence is a positive routing fact and must have an answer behind
/// it: an unmentioned target settles absent only when every configured router
/// answered. Otherwise it stays outstanding, in both the coverage map and the
/// unresolved set, so every consumer of the contribution sees the same fact.
fn complete_targets(
    mut contribution: RouteContribution,
    targets: &BTreeSet<RouteTarget>,
    all_answered: bool,
) -> RouteContribution {
    for target in targets {
        if contribution.coverage.contains_key(target) {
            continue;
        }
        if all_answered && !contribution.unresolved.contains(target) {
            contribution
                .coverage
                .insert(target.clone(), CoverageState::SettledAbsent);
        } else {
            contribution
                .coverage
                .insert(target.clone(), CoverageState::Unresolved);
            contribution.unresolved.insert(target.clone());
        }
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use fava_relay::{RelayAccess, RelaySessionKey};
    use nostr::types::RelayUrl;

    use super::*;
    use crate::RouteDestination;

    #[test]
    fn refuses_router_contribution_over_destination_bound() {
        let destinations = (0..=MAX_DESTINATIONS)
            .map(|index| {
                RouteDestination::new(
                    RelaySessionKey {
                        relay: RelayUrl::parse(&format!("wss://relay-{index}.example")).unwrap(),
                        access: RelayAccess::Public,
                    },
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
    fn chain_shortfall_overflow_reports_the_exact_discarded_count() {
        let mut composer = Composer::new(1, BTreeSet::new());
        for index in 0..(MAX_SHORTFALLS + 3) {
            composer.record(&format!("shortfall {index}"));
        }

        assert_eq!(composer.shortfalls.len(), MAX_SHORTFALLS);
        assert_eq!(composer.discarded, 3);
        assert!(
            composer
                .combined()
                .shortfalls
                .iter()
                .any(|shortfall| shortfall
                    == "chain: 3 further route shortfalls discarded beyond the 8192-entry bound")
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
