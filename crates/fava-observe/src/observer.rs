//! The synchronous, total observation open sequence.

use std::sync::{Arc, OnceLock};

use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_query::{
    Freshness, ObservationId, Query, QueryAcquisition, QueryBranchId, QueryEvaluator,
    QueryRevision, QuerySnapshot, QuerySource, RelayWithdrawal, SourceKind,
};
use fava_routing::Router;
use fava_runtime::Runtime;
use fava_subscriptions::SubscriptionPlanner;
use fava_transport::{Transport, TransportBounds, TransportDeadlines};

use crate::engine::{Engine, RelayProviders, default_bounds, default_deadlines};
use crate::error::ObserveError;
use crate::observation::Observation;
use crate::registry::Registry;
use crate::routes::{self, RouteBinding};
use crate::sources::{Coalesced, OpenSources, Projection, decorate, project, publish};

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
    registry: Arc<Registry>,
    engine: Arc<OnceLock<()>>,
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
            registry: Arc::new(Registry::default()),
            engine: Arc::new(OnceLock::new()),
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
    #[allow(
        clippy::result_large_err,
        reason = "ObserveError names the exact source role that refused; a live-relay role carries its session identity"
    )]
    pub fn open(&self, query: Query) -> Result<Observation, ObserveError> {
        validate(&query)?;
        let live = query.freshness() != Freshness::CacheOnly;
        if live {
            self.start_engine()?;
        }

        let cache = self
            .event_cache
            .open(&query)
            .map_err(|error| ObserveError::SourceOpen {
                role: SourceKind::EventCache,
                error,
            })?;
        let writes = match self.write_store.open(&query) {
            Ok(writes) => writes,
            Err(error) => {
                let mut changes = cache.changes;
                changes.close();
                return Err(ObserveError::SourceOpen {
                    role: SourceKind::WriteStore,
                    error,
                });
            }
        };
        let sources = OpenSources::new(cache, writes);

        let binding = if live {
            match routes::bind(&query, &self.routers) {
                Ok(binding) => Some(binding),
                Err(error) => {
                    sources.close();
                    return Err(error);
                }
            }
        } else {
            None
        };

        let initial = match self.evaluate_initial(&query, &sources) {
            Ok(initial) => initial,
            Err(error) => {
                sources.close();
                if let Some(binding) = binding {
                    binding.close();
                }
                return Err(error);
            }
        };

        let installation = self.registry.install(self.runtime.cancellation_token());
        let branch = QueryBranchId::ROOT;
        if let Some(binding) = binding {
            self.retain(installation.id, branch, &query, binding, &installation.cancel);
        }
        let mut initial = initial;
        decorate(&self.registry, installation.id, &mut initial.evidence);
        publish(
            self.diagnostics.as_ref(),
            &self.registry,
            installation.id,
            &initial.evidence,
        );
        let (latest, task) = project(
            &self.runtime,
            Projection {
                id: installation.id,
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
        ))
    }

    #[allow(
        clippy::result_large_err,
        reason = "ObserveError names the exact source role that refused; a live-relay role carries its session identity"
    )]
    fn evaluate_initial(
        &self,
        query: &Query,
        sources: &OpenSources,
    ) -> Result<QuerySnapshot, ObserveError> {
        let mut initial = self.evaluator.evaluate(query, &sources.snapshots)?;
        initial.revision = QueryRevision(1);
        Ok(initial)
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
            origin,
        } = binding;
        self.registry.assign(
            id,
            branch,
            routes::demand_for(id, branch, query, &plan, origin),
            origin.revision_of(&plan),
            RelayWithdrawal::RouteWithdrawn,
        );
        if let Some(session) = session {
            let task = routes::follow(
                &self.runtime,
                session,
                routes::Following {
                    id,
                    registry: Arc::clone(&self.registry),
                    query: query.clone(),
                    branch,
                    cancel: cancel.clone(),
                    revision: plan.revision,
                },
            );
            if let Some(task) = task {
                self.registry.attach(id, task);
            }
        }
    }

    #[allow(
        clippy::result_large_err,
        reason = "ObserveError names the exact source role that refused; a live-relay role carries its session identity"
    )]
    fn start_engine(&self) -> Result<(), ObserveError> {
        if self.engine.get().is_some() {
            return Ok(());
        }
        let providers = RelayProviders {
            transport: self
                .transport
                .clone()
                .ok_or_else(|| ObserveError::Relay("live queries require a transport".to_owned()))?,
            planner: self.planner.clone().ok_or_else(|| {
                ObserveError::Relay("live queries require a subscription planner".to_owned())
            })?,
            cache: self.events.clone().ok_or_else(|| {
                ObserveError::Relay("live queries require an event cache".to_owned())
            })?,
            diagnostics: Arc::clone(&self.diagnostics),
            deadlines: self.deadlines,
            bounds: self.bounds,
        };
        let mut started = Ok(());
        self.engine.get_or_init(|| {
            started = Engine::start(Arc::clone(&self.registry), providers, &self.runtime);
        });
        started.map_err(|_| ObserveError::EngineClosed)
    }
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

/// Refuse a query that cannot be represented as relay demand before opening work.
#[allow(
    clippy::result_large_err,
    reason = "ObserveError names the exact source role that refused; a live-relay role carries its session identity"
)]
fn validate(query: &Query) -> Result<(), ObserveError> {
    if query.freshness() == Freshness::CacheOnly {
        return Ok(());
    }
    if let QueryAcquisition::Explicit(relays) = query.source().acquisition()
        && relays.is_empty()
    {
        return Err(ObserveError::InvalidQuery(
            "a live explicit query requires at least one relay".to_owned(),
        ));
    }
    Ok(())
}
