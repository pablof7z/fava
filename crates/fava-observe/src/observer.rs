//! The synchronous, total observation open sequence.

mod current_account;

use std::sync::{Arc, OnceLock};

use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_query::{
    Freshness, ObservationId, Query, QueryAcquisition, QueryBranchId, QueryEvaluator,
    QueryRevision, QuerySnapshot, QuerySource, RelayWithdrawal, SourceKind,
};
use fava_routing::Router;
use fava_runtime::{Runtime, TaskName};
use fava_session::Session;
use fava_subscriptions::SubscriptionPlanner;
use fava_transport::{Transport, TransportBounds, TransportDeadlines};
use futures_util::future::select_all;

use crate::admission::ADMISSION_WINDOW;
use crate::engine::{Engine, RelayProviders};
use crate::error::ObserveError;
use crate::facts::{default_bounds, default_deadlines};
use crate::observation::Observation;
use crate::registry::Registry;
use crate::routes::{self, RouteBinding};
use crate::sources::{Coalesced, OpenSources, Projection, decorate, project, publish};

const EMPTY_ROUTER_INPUT_POLL: std::time::Duration = std::time::Duration::from_millis(10);
const CURRENT_ACCOUNT_OPEN_ATTEMPTS: usize = 64;

/// Configured universal observation owner.
///
/// The owner installs observations, retains their logical relay demand, and
/// reconciles that demand into relay work. Opening never awaits a provider.
#[derive(Clone)]
pub struct Observer {
    event_cache: Arc<dyn QuerySource>,
    write_store: Arc<dyn QuerySource>,
    evaluator: Arc<dyn QueryEvaluator>,
    coalesced: Option<Coalesced>,
    routers: Vec<Arc<dyn Router>>,
    transport: Option<Arc<dyn Transport>>,
    planner: Option<Arc<dyn SubscriptionPlanner>>,
    events: Option<Arc<dyn EventCache>>,
    diagnostics: Arc<Diagnostics>,
    runtime: Runtime,
    deadlines: TransportDeadlines,
    bounds: TransportBounds,
    admission_window: std::time::Duration,
    registry: Arc<Registry>,
    engine: Arc<OnceLock<()>>,
    session: Option<Session>,
}

impl Observer {
    /// Construct the owner from neutral provider contracts.
    #[must_use]
    pub fn new(
        event_cache: Arc<dyn QuerySource>,
        write_store: Arc<dyn QuerySource>,
        evaluator: Arc<dyn QueryEvaluator>,
    ) -> Self {
        Self {
            event_cache,
            write_store,
            evaluator,
            coalesced: None,
            routers: Vec::new(),
            transport: None,
            planner: None,
            events: None,
            diagnostics: Arc::new(Diagnostics::default()),
            runtime: Runtime::new(default_runtime_config()),
            deadlines: default_deadlines(),
            bounds: default_bounds(),
            admission_window: ADMISSION_WINDOW,
            registry: Arc::new(Registry::default()),
            engine: Arc::new(OnceLock::new()),
            session: None,
        }
    }

    /// Report current-state revisions superseded at bounded watch boundaries.
    #[must_use]
    pub fn with_coalescing(mut self, report: Coalesced) -> Self {
        self.coalesced = Some(report);
        self
    }

    /// Select the transport that performs this owner's relay work.
    #[must_use]
    pub fn with_transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Select the planner that maps this owner's logical demand onto the wire.
    #[must_use]
    pub fn with_subscription_planner(mut self, planner: Arc<dyn SubscriptionPlanner>) -> Self {
        self.planner = Some(planner);
        self
    }

    /// Select the cache that admits relay-served events.
    #[must_use]
    pub fn with_event_cache(mut self, events: Arc<dyn EventCache>) -> Self {
        self.events = Some(events);
        self
    }

    /// Bind current-account query dependencies to this runtime session.
    #[must_use]
    pub fn with_session(mut self, session: Session) -> Self {
        self.session = Some(session);
        self
    }

    /// Publish this owner's facts into the selected bounded diagnostics.
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: Arc<Diagnostics>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// Select the ordered automatic routers for this owner's route sessions.
    #[must_use]
    pub fn with_routers(mut self, routers: Vec<Arc<dyn Router>>) -> Self {
        self.routers = routers;
        self
    }

    /// Execute this owner's work on the engine's runtime.
    #[must_use]
    pub fn with_runtime(mut self, runtime: Runtime) -> Self {
        self.runtime = runtime;
        self
    }

    /// Apply Fava-owned deadlines to every relay session this owner acquires.
    #[must_use]
    pub const fn with_deadlines(mut self, deadlines: TransportDeadlines) -> Self {
        self.deadlines = deadlines;
        self
    }

    /// Apply Fava-owned queue and frame bounds to every relay session.
    #[must_use]
    pub const fn with_bounds(mut self, bounds: TransportBounds) -> Self {
        self.bounds = bounds;
        self
    }

    /// Batch unsent relay demand for this long before compiling one cohort.
    ///
    /// The window is anchored at the first uncovered demand and never slides.
    #[must_use]
    pub const fn with_admission_window(mut self, window: std::time::Duration) -> Self {
        self.admission_window = window;
        self
    }

    /// Install one observation and return an immediately readable handle.
    ///
    /// The sequence is total and synchronous: source boundary, route binding,
    /// logical demand, initial evaluation, installation, handle release. Relay
    /// work is enqueued for the reconciliation owner and never awaited here.
    ///
    /// # Errors
    ///
    /// Returns [`ObserveError`] when the query is invalid, a local source
    /// refuses to open, a route plan cannot be produced, or initial evaluation
    /// fails. Every provisionally opened resource is released.
    pub fn open(&self, query: Query) -> Result<Observation, ObserveError> {
        if query.depends_on_current_account() {
            let session = self
                .session
                .clone()
                .ok_or(ObserveError::SessionUnconfigured)?;
            return self.open_current_account(query, session);
        }
        self.open_concrete(query, None)
    }

    #[allow(
        clippy::result_large_err,
        reason = "ObserveError names the exact source role that refused; a live-relay role carries its session identity"
    )]
    fn open_current_account(
        &self,
        query: Query,
        session: Session,
    ) -> Result<Observation, ObserveError> {
        let installation = self.registry.install(self.runtime.cancellation_token());
        let changes = session.subscribe();
        let (child, current, current_revision) =
            match self.open_stable_current_child(&query, &session, installation.id) {
                Ok(opened) => opened,
                Err(error) => {
                    self.registry.withdraw(installation.id);
                    return Err(error);
                }
            };
        let child_current = child.current();
        let initial = current_account::revision(child_current.as_ref(), 1);
        let (latest_tx, latest) = tokio::sync::watch::channel(Arc::clone(&initial));
        let (synchronized_tx, synchronized) =
            tokio::sync::watch::channel(((current, current_revision), initial));
        let synchronization_session = session.clone();
        let task = self.runtime.spawn_cancellable(
            TaskName("observe.current-account"),
            installation.cancel.clone(),
            current_account::run(current_account::Follow {
                observer: self.clone(),
                parent: installation.id,
                query,
                session,
                changes,
                child,
                current,
                current_revision,
                latest: latest_tx,
                synchronized: synchronized_tx,
            }),
        );
        let Ok(task) = task else {
            self.registry.withdraw(installation.id);
            return Err(ObserveError::EngineClosed);
        };
        self.registry.attach(installation.id, task);
        Ok(Observation::new(
            installation.id,
            Arc::clone(&self.registry),
            latest,
            installation.cancel,
            self.coalesced.clone(),
            Some(Arc::clone(&self.diagnostics)),
            Some((synchronization_session, synchronized)),
        ))
    }

    #[allow(
        clippy::result_large_err,
        reason = "ObserveError names the exact source role that refused; a live-relay role carries its session identity"
    )]
    fn open_stable_current_child(
        &self,
        query: &Query,
        session: &Session,
        parent: ObservationId,
    ) -> Result<(Observation, Option<fava_query::PublicKey>, u64), ObserveError> {
        for _ in 0..CURRENT_ACCOUNT_OPEN_ATTEMPTS {
            let (account, revision) = session.current_account_snapshot();
            let child =
                self.open_concrete(current_account::bind(query.clone(), account), Some(parent))?;
            let activated = session.if_current_account((account, revision), || {
                if !self.registry.activate_child(parent, child.id()) {
                    return None;
                }
                let snapshot = child.current();
                publish(
                    self.diagnostics.as_ref(),
                    &self.registry,
                    child.id(),
                    parent,
                    &snapshot.evidence,
                );
                Some(())
            });
            match activated {
                Some(Some(())) => return Ok((child, account, revision)),
                Some(None) => {
                    child.close();
                    return Err(ObserveError::EngineClosed);
                }
                None => child.close(),
            }
        }
        Err(ObserveError::CurrentAccountUnstable)
    }

    #[allow(
        clippy::result_large_err,
        clippy::too_many_lines,
        reason = "one all-or-nothing open sequence keeps every provisional source, route, generation, and cleanup edge visible together"
    )]
    fn open_concrete(
        &self,
        query: Query,
        diagnostic: Option<ObservationId>,
    ) -> Result<Observation, ObserveError> {
        let live = query.freshness() != Freshness::CacheOnly && !query.matches_nothing();

        let cache = self
            .event_cache
            .open(&query)
            .map_err(|error| ObserveError::SourceOpen {
                role: Box::new(SourceKind::EventCache),
                error: Box::new(error),
            })?;
        let writes = match self.write_store.open(&query) {
            Ok(writes) => writes,
            Err(error) => {
                let mut changes = cache.changes;
                changes.close();
                return Err(ObserveError::SourceOpen {
                    role: Box::new(SourceKind::WriteStore),
                    error: Box::new(error),
                });
            }
        };
        let mut sources = OpenSources::new(cache, writes);

        let mut binding = if live {
            match routes::bind(&query, &self.routers, |input| self.local_snapshot(input)) {
                Ok(binding) => Some(binding),
                Err(error) => {
                    sources.close();
                    return Err(error);
                }
            }
        } else {
            None
        };

        let cancel = self.runtime.cancellation_token();
        let installation = if let Some(public) = diagnostic {
            if let Some(installation) = self.registry.install_child(cancel, public) {
                installation
            } else {
                sources.close();
                if let Some(session) = binding
                    .as_mut()
                    .and_then(|binding| binding.session.as_mut())
                {
                    session.close();
                }
                return Err(ObserveError::EngineClosed);
            }
        } else {
            self.registry.install(cancel)
        };
        let branch = QueryBranchId::ROOT;
        if let Some(mut binding) = binding {
            self.remove_fresh_sources(&query, installation.id, branch, &mut binding);
            if (!binding.plan.destinations.is_empty() || binding.session.is_some())
                && let Err(error) = self.start_engine()
            {
                sources.close();
                self.registry.withdraw(installation.id);
                return Err(error);
            }
            self.retain(
                installation.id,
                branch,
                &query,
                binding,
                &installation.cancel,
            );
        }
        sources.refresh_live(&self.registry, installation.id);

        let initial = match self.evaluate_initial(&query, &sources) {
            Ok(initial) => initial,
            Err(error) => {
                sources.close();
                self.registry.withdraw(installation.id);
                return Err(error);
            }
        };
        let diagnostic_id = diagnostic.is_none().then_some(installation.id);
        let mut initial = initial;
        decorate(&self.registry, installation.id, &mut initial.evidence);
        if let Some(diagnostic_id) = diagnostic_id {
            publish(
                self.diagnostics.as_ref(),
                &self.registry,
                installation.id,
                diagnostic_id,
                &initial.evidence,
            );
        }
        let (latest, task) = project(
            &self.runtime,
            Projection {
                id: installation.id,
                diagnostic_id,
                registry: Arc::clone(&self.registry),
                diagnostics: Arc::clone(&self.diagnostics),
                evaluator: Arc::clone(&self.evaluator),
                coalesced: self.coalesced.clone(),
                cancel: installation.cancel.clone(),
                woken: installation.woken,
            },
            query,
            sources,
            initial,
        );
        if let Some(task) = task {
            self.registry.attach(installation.id, task);
        }
        Ok(Observation::new(
            installation.id,
            Arc::clone(&self.registry),
            latest,
            installation.cancel,
            self.coalesced.clone(),
            None,
            None,
        ))
    }

    /// Evaluate automatic routing from current local router inputs without
    /// opening observations or relay work.
    ///
    /// # Errors
    ///
    /// Returns [`ObserveError`] when a local source or router refuses its
    /// exact preview input.
    pub fn preview_routes(&self, query: &Query) -> Result<fava_routing::RoutePlan, ObserveError> {
        let request = fava_routing::RouteRequest::Read(query.clone());
        match query.source().acquisition() {
            QueryAcquisition::Explicit(relays) => {
                fava_routing::RoutePlan::explicit(relays.iter().cloned(), &request.targets())
                    .map_err(|error| ObserveError::Relay(error.to_string()))
            }
            QueryAcquisition::Automatic => {
                let declared = fava_routing::queries(&self.routers, &request)
                    .map_err(|error| ObserveError::Relay(error.to_string()))?;
                let inputs = declared
                    .iter()
                    .map(|queries| {
                        queries
                            .iter()
                            .map(|input| self.local_snapshot(input))
                            .collect()
                    })
                    .collect::<Result<Vec<Vec<_>>, _>>()?;
                fava_routing::preview(&self.routers, &request, &inputs)
                    .map_err(|error| ObserveError::Relay(error.to_string()))
            }
        }
    }

    /// Remove only sources backed by one still-fresh exact proven completion.
    ///
    /// The decision is made once during open.  No retained timer owns a later
    /// recheck, so a `MaxAge` observation keeps local replacements but cannot
    /// restart relay work merely by becoming old.
    fn remove_fresh_sources(
        &self,
        query: &Query,
        id: ObservationId,
        branch: QueryBranchId,
        binding: &mut RouteBinding,
    ) {
        let Freshness::MaxAge(age) = query.freshness() else {
            return;
        };
        let Some(cache) = &self.events else {
            return;
        };
        let filter = fava_subscriptions::demand_for_query(id, branch, query).filter;
        let opened_at = nostr::types::Timestamp::now();
        binding.plan.destinations.retain(|session, _| {
            let Ok(Some(coverage)) = cache.source_coverage(session, &filter) else {
                return true;
            };
            opened_at
                .as_secs()
                .saturating_sub(coverage.completed_at.as_secs())
                > age.as_secs()
        });
    }

    fn evaluate_initial(
        &self,
        query: &Query,
        sources: &OpenSources,
    ) -> Result<QuerySnapshot, ObserveError> {
        let mut initial = self.evaluator.evaluate(query, &sources.snapshots)?;
        initial.revision = QueryRevision(1);
        Ok(initial)
    }

    fn local_snapshot(&self, query: &Query) -> Result<QuerySnapshot, ObserveError> {
        let cache = self
            .event_cache
            .open(query)
            .map_err(|error| ObserveError::SourceOpen {
                role: Box::new(SourceKind::EventCache),
                error: Box::new(error),
            })?;
        let writes = match self.write_store.open(query) {
            Ok(writes) => writes,
            Err(error) => {
                let mut changes = cache.changes;
                changes.close();
                return Err(ObserveError::SourceOpen {
                    role: Box::new(SourceKind::WriteStore),
                    error: Box::new(error),
                });
            }
        };
        let sources = OpenSources::new(cache, writes);
        let result = self.evaluate_initial(query, &sources);
        sources.close();
        result
    }

    fn retain(
        &self,
        id: ObservationId,
        branch: QueryBranchId,
        query: &Query,
        binding: RouteBinding,
        cancel: &fava_runtime::CancellationToken,
    ) {
        let RouteBinding {
            plan,
            session,
            inputs,
            origin,
        } = binding;
        self.registry.assign(
            id,
            branch,
            *query.access(),
            routes::demand_for(id, branch, query, &plan, origin),
            origin.revision_of(&plan),
            RelayWithdrawal::RouteWithdrawn,
        );
        if let Some(session) = session {
            self.follow_routes(
                FollowedRoute {
                    id,
                    branch,
                    request: query.clone(),
                    plan,
                    origin,
                    inputs,
                    session,
                },
                cancel,
            );
        }
    }

    fn follow_routes(&self, route: FollowedRoute<Query>, cancel: &fava_runtime::CancellationToken) {
        let FollowedRoute {
            id,
            branch,
            request,
            plan,
            origin,
            inputs: input_queries,
            session,
        } = route;
        let mut inputs = Vec::with_capacity(input_queries.len());
        for input in input_queries {
            let Ok(observation) = self.open(input) else {
                return;
            };
            inputs.push(observation);
        }
        let task = self.runtime.spawn_cancellable(
            TaskName("observe.router-inputs"),
            cancel.clone(),
            follow_route_inputs(
                FollowedRoute {
                    id,
                    branch,
                    request,
                    plan,
                    origin,
                    inputs,
                    session,
                },
                Arc::clone(&self.registry),
            ),
        );
        if let Ok(task) = task {
            self.registry.attach(id, task);
        }
    }

    fn start_engine(&self) -> Result<(), ObserveError> {
        if self.engine.get().is_some() {
            return Ok(());
        }
        let providers = RelayProviders {
            transport: self.transport.clone().ok_or_else(|| {
                ObserveError::Relay("live queries require a transport".to_owned())
            })?,
            planner: self.planner.clone().ok_or_else(|| {
                ObserveError::Relay("live queries require a subscription planner".to_owned())
            })?,
            cache: self.events.clone().ok_or_else(|| {
                ObserveError::Relay("live queries require an event cache".to_owned())
            })?,
            diagnostics: Arc::clone(&self.diagnostics),
            deadlines: self.deadlines,
            bounds: self.bounds,
            admission_window: self.admission_window,
        };
        let mut started = Ok(());
        self.engine.get_or_init(|| {
            started = Engine::start(Arc::clone(&self.registry), providers, &self.runtime);
        });
        started
    }
}

/// One route the engine is following for one observation branch, generic
/// over whether its declared axes are still bare [`Query`] values or have
/// already been opened into [`Observation`]s.
struct FollowedRoute<I> {
    id: ObservationId,
    branch: QueryBranchId,
    request: Query,
    plan: fava_routing::RoutePlan,
    origin: routes::Origin,
    inputs: Vec<I>,
    session: Box<dyn fava_routing::RouterSession>,
}

async fn follow_route_inputs(route: FollowedRoute<Observation>, registry: Arc<Registry>) {
    let FollowedRoute {
        id,
        branch,
        request: query,
        mut plan,
        origin,
        mut inputs,
        mut session,
    } = route;
    loop {
        let snapshots = inputs
            .iter()
            .map(|observation| observation.current().as_ref().clone())
            .collect();
        let Ok(contribution) = session.replace(Arc::new(plan.clone()), snapshots) else {
            break;
        };
        if inputs.is_empty()
            && fava_routing::RoutePlan::from_contribution(plan.revision, &contribution)
                .is_ok_and(|current| current == plan)
        {
            tokio::time::sleep(EMPTY_ROUTER_INPUT_POLL).await;
            continue;
        }
        let Some(revision) = plan.revision.checked_add(1) else {
            break;
        };
        let Ok(next) = fava_routing::RoutePlan::from_contribution(revision, &contribution) else {
            break;
        };
        plan = next;
        registry.assign(
            id,
            branch,
            *query.access(),
            routes::demand_for(id, branch, &query, &plan, origin),
            origin.revision_of(&plan),
            RelayWithdrawal::RouteWithdrawn,
        );
        if inputs.is_empty() {
            // A router with no declared queries can still replace its whole
            // contribution (for example, after a delayed local policy
            // update). There is no observation change to await in that case,
            // so re-evaluate it at a bounded cadence instead of parking the
            // route session forever.
            tokio::time::sleep(EMPTY_ROUTER_INPUT_POLL).await;
        } else {
            let pending = inputs
                .iter_mut()
                .map(|input| Box::pin(input.changed()))
                .collect::<Vec<_>>();
            let (outcome, _, _) = select_all(pending).await;
            if outcome.is_err() {
                break;
            }
        }
    }
    session.close();
}

/// The runtime an owner constructs when the assembly supplies none.
fn default_runtime_config() -> fava_runtime::RuntimeConfig {
    fava_runtime::RuntimeConfig {
        default_channel_depth: nonzero(1_024),
        max_tasks: nonzero(65_536),
        max_provider_operations: nonzero(4_096),
    }
}

fn nonzero(value: usize) -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(value).expect("constant is non-zero")
}
