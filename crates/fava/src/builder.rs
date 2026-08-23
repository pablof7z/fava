//! Static assembly of one Fava engine from selected providers.

use std::num::NonZeroUsize;
use std::sync::Arc;

use fava_delivery::DeliveryPolicy;
use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_observe::Observer;
use fava_publication::Publication;
use fava_publisher::Publisher;
use fava_query::{QueryEvaluator, QuerySource};
use fava_routing::Router;
use fava_runtime::{Runtime, RuntimeConfig};
use fava_session::{Session, SessionError};
use fava_signer::Signer;
use fava_subscriptions::SubscriptionPlanner;
use fava_transport::Transport;
use fava_write::ReplaceableEventMaterializer;
use fava_write_store::WriteStore;
use thiserror::Error;

use crate::Fava;

/// Static assembly builder. No provider is silently selected.
#[derive(Default)]
pub struct FavaBuilder {
    event_cache: Option<Arc<dyn EventCache>>,
    runtime: Option<Runtime>,
    write_store: Option<Arc<dyn WriteStore>>,
    evaluator: Option<Arc<dyn QueryEvaluator>>,
    subscription_planner: Option<Arc<dyn SubscriptionPlanner>>,
    transport: Option<Arc<dyn Transport>>,
    routers: Vec<Arc<dyn Router>>,
    signers: Vec<Arc<dyn Signer>>,
    materializers: Vec<Arc<dyn ReplaceableEventMaterializer>>,
    publisher: Option<Arc<dyn Publisher>>,
    delivery: Option<Arc<dyn DeliveryPolicy>>,
    diagnostics_capacity: Option<NonZeroUsize>,
}

impl FavaBuilder {
    /// Retain at most `capacity` diagnostic facts per category. Defaults to 256.
    #[must_use]
    pub const fn diagnostics_capacity(mut self, capacity: NonZeroUsize) -> Self {
        self.diagnostics_capacity = Some(capacity);
        self
    }

    /// Select one event-cache provider.
    #[must_use]
    pub fn event_cache<T>(mut self, cache: Arc<T>) -> Self
    where
        T: EventCache + 'static,
    {
        self.event_cache = Some(cache);
        self
    }

    /// Select one write-store provider.
    #[must_use]
    pub fn write_store<T>(mut self, store: Arc<T>) -> Self
    where
        T: WriteStore + 'static,
    {
        self.write_store = Some(store);
        self
    }

    /// Select one local query evaluator.
    #[must_use]
    pub fn query_evaluator<T>(mut self, evaluator: Arc<T>) -> Self
    where
        T: QueryEvaluator + 'static,
    {
        self.evaluator = Some(evaluator);
        self
    }

    /// Select one exact subscription planner.
    #[must_use]
    pub fn subscription_planner<T>(mut self, planner: Arc<T>) -> Self
    where
        T: SubscriptionPlanner + 'static,
    {
        self.subscription_planner = Some(planner);
        self
    }

    /// Select one relay transport provider.
    #[must_use]
    pub fn transport<T>(mut self, transport: Arc<T>) -> Self
    where
        T: Transport + 'static,
    {
        self.transport = Some(transport);
        self
    }

    /// Append one automatic router in application-selected order.
    #[must_use]
    pub fn router<T>(mut self, router: Arc<T>) -> Self
    where
        T: Router + 'static,
    {
        self.routers.push(router);
        self
    }

    /// Append already-erased automatic routers in application-selected order.
    #[must_use]
    pub fn routers(mut self, routers: impl IntoIterator<Item = Arc<dyn Router>>) -> Self {
        self.routers.extend(routers);
        self
    }

    /// Register one signer for its exact public key.
    #[must_use]
    pub fn signer<T>(mut self, signer: Arc<T>) -> Self
    where
        T: Signer + 'static,
    {
        self.signers.push(signer);
        self
    }

    /// Register already-erased signers for their exact public keys.
    #[must_use]
    pub fn signers(mut self, signers: impl IntoIterator<Item = Arc<dyn Signer>>) -> Self {
        self.signers.extend(signers);
        self
    }

    /// Select one semantic materializer for its exact replaceable kind.
    #[must_use]
    pub fn materializer<T>(mut self, materializer: Arc<T>) -> Self
    where
        T: ReplaceableEventMaterializer + 'static,
    {
        self.materializers.push(materializer);
        self
    }

    /// Select already-erased semantic materializers.
    #[must_use]
    pub fn materializers(
        mut self,
        materializers: impl IntoIterator<Item = Arc<dyn ReplaceableEventMaterializer>>,
    ) -> Self {
        self.materializers.extend(materializers);
        self
    }

    /// Select one one-attempt publisher.
    #[must_use]
    pub fn publisher<T>(mut self, publisher: Arc<T>) -> Self
    where
        T: Publisher + 'static,
    {
        self.publisher = Some(publisher);
        self
    }

    /// Select one delivery-decision policy.
    #[must_use]
    pub fn delivery_policy<T>(mut self, delivery: Arc<T>) -> Self
    where
        T: DeliveryPolicy + 'static,
    {
        self.delivery = Some(delivery);
        self
    }

    /// Select the execution owner every Fava-started task is registered with.
    #[must_use]
    pub fn runtime(mut self, runtime: Runtime) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Validate the complete Slice 1 assembly.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] naming the first required provider role that was
    /// not selected.
    pub fn build(self) -> Result<Fava, BuildError> {
        let event_cache = self.event_cache.ok_or(BuildError::MissingEventCache)?;
        let write_store = self.write_store.ok_or(BuildError::MissingWriteStore)?;
        let evaluator = self.evaluator.ok_or(BuildError::MissingQueryEvaluator)?;
        let event_source: Arc<dyn QuerySource> = event_cache.clone();
        let write_source: Arc<dyn QuerySource> = write_store.clone();
        let diagnostics = self.diagnostics_capacity.map_or_else(
            || Arc::new(Diagnostics::default()),
            |capacity| Arc::new(Diagnostics::bounded(capacity)),
        );
        let runtime = self.runtime.unwrap_or_else(default_runtime);
        let publication_selected = self.publisher.is_some()
            || self.delivery.is_some()
            || !self.signers.is_empty()
            || !self.materializers.is_empty();
        let session = Session::new(self.signers)?;
        let publication = if publication_selected {
            let publisher = self.publisher.ok_or(BuildError::MissingPublisher)?;
            let delivery = self.delivery.ok_or(BuildError::MissingDeliveryPolicy)?;
            let transport = self
                .transport
                .clone()
                .ok_or(BuildError::MissingPublicationTransport)?;
            let publication = Publication::new(
                write_store.clone(),
                event_source.clone(),
                evaluator.clone(),
                self.materializers,
                session.clone(),
                publisher,
                delivery,
                transport,
                self.routers.clone(),
            )
            .map_err(|error| BuildError::Publication(error.to_string()))?;
            publication
                .recover()
                .map_err(|error| BuildError::Publication(error.to_string()))?;
            Some(publication)
        } else {
            None
        };
        let mut observer = Observer::new(event_source, write_source, evaluator)
            .with_event_cache(event_cache)
            .with_diagnostics(Arc::clone(&diagnostics))
            .with_routers(self.routers.clone())
            .with_runtime(runtime);
        if let Some(transport) = self.transport {
            observer = observer.with_transport(transport);
        }
        if let Some(planner) = self.subscription_planner {
            observer = observer.with_subscription_planner(planner);
        }
        Ok(Fava {
            observer,
            write_store,
            diagnostics,
            routers: self.routers,
            session,
            publication,
        })
    }
}

/// The execution owner an assembly gets when it selects none.
fn default_runtime() -> Runtime {
    Runtime::new(RuntimeConfig {
        default_channel_depth: nonzero(1_024),
        max_tasks: nonzero(65_536),
        max_provider_operations: nonzero(4_096),
    })
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("constant is non-zero")
}

/// Static assembly refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BuildError {
    /// No event-cache authority was selected.
    #[error("Fava assembly requires one event-cache provider")]
    MissingEventCache,
    /// No write-store authority was selected.
    #[error("Fava assembly requires one write-store provider")]
    MissingWriteStore,
    /// No local query evaluator was selected.
    #[error("Fava assembly requires one query evaluator")]
    MissingQueryEvaluator,
    /// Publication selected without a publisher.
    #[error("Fava publication assembly requires one publisher")]
    MissingPublisher,
    /// Publication selected without a delivery policy.
    #[error("Fava publication assembly requires one delivery policy")]
    MissingDeliveryPolicy,
    /// Publication selected without a transport.
    #[error("Fava publication assembly requires one transport")]
    MissingPublicationTransport,
    /// Publication providers or durable recovery were invalid.
    #[error("Fava publication assembly failed: {0}")]
    Publication(String),
    /// Runtime signer seed selection was invalid.
    #[error(transparent)]
    Session(#[from] SessionError),
}
