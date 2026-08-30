use std::collections::BTreeSet;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use fava_query::{Query, QuerySnapshot};

use crate::{
    CoverageState, RouteContribution, RoutePlan, RouteRequest, RouteTarget, Router, RouterError,
    RouterSession, merge_coverage,
};

const CHAIN_ACCUMULATION_FACTOR: usize = 32;
const MAX_DESTINATIONS: usize = 256;
const MAX_TARGETS: usize = 256;
const MAX_COVERAGE: usize = 256;
const MAX_COVERED_SESSIONS: usize = 256;
const MAX_SHORTFALLS: usize = 256;
const MAX_TEXT_BYTES: usize = 4_096;

/// Declare the ordered complete query set required by a router chain.
///
/// # Errors
///
/// Returns [`RouterError`] when any router in the chain refuses to declare
/// its queries.
pub fn queries(
    routers: &[Arc<dyn Router>],
    request: &RouteRequest,
) -> Result<Vec<Vec<Query>>, RouterError> {
    validate_names(routers)?;
    let composer = Composer::new(routers.len(), request.targets());
    routers
        .iter()
        .enumerate()
        .map(|(index, router)| {
            let upstream = composer.upstream_plan(index, 1);
            isolate("declaring queries", || router.queries(request, &upstream))
        })
        .collect()
}

/// Evaluate an ordered router chain from complete local query snapshots.
///
/// # Errors
///
/// Returns [`RouterError`] when the chain cannot declare or evaluate a
/// coherent plan from the supplied inputs.
pub fn preview(
    routers: &[Arc<dyn Router>],
    request: &RouteRequest,
    inputs: &[Vec<QuerySnapshot>],
) -> Result<RoutePlan, RouterError> {
    validate_names(routers)?;
    if inputs.len() != routers.len() {
        return Err(RouterError::Refused(
            "router input groups do not match routers".to_owned(),
        ));
    }
    let mut composer = Composer::new(routers.len(), request.targets());
    for (index, router) in routers.iter().enumerate() {
        let upstream = composer.upstream_plan(index, 1);
        let contribution = isolate("declaring queries", || router.queries(request, &upstream))
            .and_then(|declared| {
                exact_inputs(&declared, &inputs[index])?;
                isolate("previewing", || {
                    router.preview(request, &upstream, &inputs[index])
                })
            });
        match contribution.and_then(|value| attribute(value, router.name())) {
            Ok(value) => composer.accept(index, router.name(), value),
            Err(error) => composer.record(&bounded_error(router.name(), &error)),
        }
    }
    composer.plan(1)
}

/// Open an ordered router chain from complete engine-owned snapshots.
///
/// # Errors
///
/// Returns [`RouterError`] when the chain cannot declare a coherent plan or
/// any router refuses to open its session from the supplied inputs.
pub fn open(
    routers: &[Arc<dyn Router>],
    request: &RouteRequest,
    inputs: &[Vec<QuerySnapshot>],
) -> Result<Box<dyn RouterSession>, RouterError> {
    validate_names(routers)?;
    if inputs.len() != routers.len() {
        return Err(RouterError::Refused(
            "router input groups do not match routers".to_owned(),
        ));
    }
    let mut composer = Composer::new(routers.len(), request.targets());
    let mut sessions = Vec::with_capacity(routers.len());
    let mut counts = Vec::with_capacity(routers.len());
    for (index, router) in routers.iter().enumerate() {
        let upstream = Arc::new(composer.upstream_plan(index, 1));
        let result = isolate("declaring queries", || router.queries(request, &upstream));
        let result = match result {
            Ok(declared) => {
                counts.push(declared.len());
                exact_inputs(&declared, &inputs[index]).and_then(|()| {
                    isolate("opening", || {
                        router.open(
                            request.clone(),
                            Arc::clone(&upstream),
                            inputs[index].clone(),
                        )
                    })
                })
            }
            Err(error) => {
                counts.push(0);
                Err(error)
            }
        };
        match result {
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
    Ok(Box::new(OpenedChain {
        sessions,
        composer,
        counts,
        closed: false,
    }))
}

fn exact_inputs(declared: &[Query], inputs: &[QuerySnapshot]) -> Result<(), RouterError> {
    (declared.len() == inputs.len())
        .then_some(())
        .ok_or_else(|| {
            RouterError::Refused("router input count does not match declaration".to_owned())
        })
}

fn current_contribution(
    session: &mut dyn RouterSession,
    router: &str,
) -> Result<RouteContribution, RouterError> {
    isolate("reading its current contribution", || Ok(session.current()))
        .and_then(|value| attribute(value, router))
}

fn close_session(session: &mut dyn RouterSession) {
    drop(isolate("closing", || {
        session.close();
        Ok(())
    }));
}

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

struct OpenedChain {
    sessions: Vec<Option<(String, Box<dyn RouterSession>)>>,
    composer: Composer,
    counts: Vec<usize>,
    closed: bool,
}

impl RouterSession for OpenedChain {
    fn current(&self) -> RouteContribution {
        self.composer.combined()
    }

    fn replace(
        &mut self,
        _upstream: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<RouteContribution, RouterError> {
        if self.closed {
            return Err(RouterError::Closed);
        }
        if inputs.len() != self.counts.iter().sum::<usize>() {
            return Err(RouterError::Refused(
                "router replacement input count changed".to_owned(),
            ));
        }
        let mut cursor = 0;
        for index in 0..self.sessions.len() {
            let count = self.counts[index];
            let next = inputs[cursor..cursor + count].to_vec();
            cursor += count;
            let upstream = Arc::new(self.composer.upstream_plan(index, 1));
            let Some((name, session)) = &mut self.sessions[index] else {
                continue;
            };
            match isolate("replacing", || session.replace(upstream, next))
                .and_then(|value| attribute(value, name))
            {
                Ok(value) => self.composer.accept(index, name, value),
                Err(error) => self.composer.record(&bounded_error(name, &error)),
            }
        }
        Ok(self.current())
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        for (_, session) in self.sessions.iter_mut().flatten() {
            close_session(session.as_mut());
        }
    }
}

impl Drop for OpenedChain {
    fn drop(&mut self) {
        self.close();
    }
}

struct Composer {
    contributions: Vec<RouteContribution>,
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
    fn record(&mut self, shortfall: &str) {
        if self.shortfalls.len() < MAX_SHORTFALLS {
            self.shortfalls.push(shortfall.to_owned());
        } else {
            self.discarded = self.discarded.saturating_add(1);
        }
    }
    fn accept(&mut self, index: usize, router: &str, contribution: RouteContribution) {
        let previous = std::mem::replace(&mut self.contributions[index], contribution);
        let answered = std::mem::replace(&mut self.answered[index], true);
        if let Err(error) = validate_combined(&self.merged()) {
            self.contributions[index] = previous;
            self.answered[index] = answered;
            self.record(&bounded_error(router, &error));
        }
    }
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
    fn combined(&self) -> RouteContribution {
        let mut combined = self.merged();
        combined.shortfalls.extend(self.shortfalls.iter().cloned());
        let mut discarded = self.discarded;
        let maximum = MAX_SHORTFALLS * CHAIN_ACCUMULATION_FACTOR;
        if combined.shortfalls.len() >= maximum {
            discarded = discarded.saturating_add(combined.shortfalls.len() - maximum + 1);
            combined.shortfalls.truncate(maximum - 1);
        }
        if discarded > 0 {
            combined.shortfalls.push(format!("chain: {discarded} further route shortfalls discarded beyond the {maximum}-entry bound"));
        }
        combined
    }
    fn upstream_plan(&self, index: usize, revision: u64) -> RoutePlan {
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

fn attribute(
    mut contribution: RouteContribution,
    router: &str,
) -> Result<RouteContribution, RouterError> {
    for destination in &mut contribution.destinations {
        destination.set_router(router);
    }
    validate_contribution(&contribution, 1)?;
    Ok(contribution)
}
fn bounded_error(router: &str, error: &RouterError) -> String {
    let mut text = format!("{router}: {error}");
    text.truncate(MAX_TEXT_BYTES);
    text
}
fn validate_names(routers: &[Arc<dyn Router>]) -> Result<(), RouterError> {
    let mut names = BTreeSet::new();
    for router in routers {
        if router.name().is_empty() {
            return Err(RouterError::Refused("router name is empty".to_owned()));
        }
        bounded("router name", router.name().len(), MAX_TEXT_BYTES)?;
        if !names.insert(router.name()) {
            return Err(RouterError::Refused(format!(
                "duplicate router name: {}",
                router.name()
            )));
        }
    }
    Ok(())
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
        bounded("route reason", destination.reason.len(), MAX_TEXT_BYTES)?;
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
        bounded("route shortfall", shortfall.len(), MAX_TEXT_BYTES)?;
    }
    Ok(())
}
pub(crate) fn validate_combined(contribution: &RouteContribution) -> Result<(), RouterError> {
    validate_contribution(contribution, CHAIN_ACCUMULATION_FACTOR)
}
fn bounded(label: &str, actual: usize, maximum: usize) -> Result<(), RouterError> {
    if actual > maximum {
        return Err(RouterError::Refused(format!(
            "{label} exceed bound: {actual} > {maximum}"
        )));
    }
    Ok(())
}
fn combine(contributions: &[RouteContribution]) -> RouteContribution {
    let mut destinations = Vec::new();
    let mut coverage = std::collections::BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    let mut shortfalls = Vec::new();
    for contribution in contributions {
        destinations.extend(contribution.destinations.iter().cloned());
        for (target, state) in &contribution.coverage {
            merge_coverage(
                coverage
                    .entry(target.clone())
                    .or_insert(CoverageState::Unresolved),
                state,
            );
        }
        unresolved.extend(contribution.unresolved.iter().cloned());
        shortfalls.extend(contribution.shortfalls.iter().cloned());
    }
    RouteContribution {
        destinations,
        coverage,
        unresolved,
        shortfalls,
    }
}
fn complete_targets(
    mut contribution: RouteContribution,
    targets: &BTreeSet<RouteTarget>,
    all_answered: bool,
) -> RouteContribution {
    for target in targets {
        contribution
            .coverage
            .entry(target.clone())
            .or_insert_with(|| {
                if all_answered {
                    CoverageState::SettledAbsent
                } else {
                    contribution.unresolved.insert(target.clone());
                    CoverageState::Unresolved
                }
            });
    }
    contribution
}
