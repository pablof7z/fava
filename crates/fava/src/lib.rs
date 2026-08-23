//! Thin Rust facade over the selected Fava provider assembly.

mod publication;
mod query_source;
mod session;

use std::num::NonZeroUsize;
use std::sync::Arc;

use fava_delivery::DeliveryPolicy;
use fava_diagnostics::Diagnostics;
pub use fava_diagnostics::{
    BoundKind, DiagnosticsSnapshot, DroppedFacts, LimitDiagnostic, LimitScope,
    LogicalDemandDiagnostic, ObservationWireBinding, ProviderDiagnostic, ProviderKind,
    ProviderOperation, ProviderOperationState, QueryDiagnostic, RelayDiagnostic, RelaySessionState,
    WireSubscriptionDiagnostic, WriteDiagnostic, WriteStall,
};
use fava_event_cache::EventCache;
use fava_observe::Observer;
pub use fava_observe::{Observation, ObservationClosed, ObserveError};
use fava_publication::Publication;
pub use fava_publication::PublicationError;
use fava_publisher::Publisher;
pub use fava_query::{
    EventRecord, Freshness, Query, QueryRevision, QuerySnapshot, ResultAuthority, SingleLetterTag,
};
use fava_query::{QueryEvaluator, QuerySource};
pub use fava_routing::RoutePlan;
use fava_routing::{RouteRequest, Router};
pub use fava_runtime::{Runtime, RuntimeConfig};
use fava_session::Session;
pub use fava_session::SessionError;
use fava_signer::Signer;
pub use fava_state::{EventCoordinate, RelayUrl};
use fava_subscriptions::SubscriptionPlanner;
use fava_transport::Transport;
pub use fava_write::{
    Event, EventBuildError, EventBuilder, EventValue, Kind, MaterializationId, PublicKey, Receipt,
    ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Tag, Timestamp, UnsignedEvent, WriteId, WriteIntentError,
    WriteRouting,
};
use fava_write_store::WriteStore;
pub use fava_write_store::WriteStoreError;
pub use publication::{PublishAs, PublishError, PublishTo, Write, all, at_least};
use thiserror::Error;
use tokio::sync::broadcast;

/// Built engine instance for the selected local-source assembly.
///
/// Application publication does not expose neutral custody inputs or results:
///
/// ```compile_fail
/// use fava::{AcceptedWrite, WriteIntent, WritePayload};
/// ```
///
/// A neutral intent cannot enter the application publication door:
///
/// ```compile_fail
/// fn old_publication_door(fava: &fava::Fava, intent: fava_write::WriteIntent) {
///     let _ = fava.publish(intent);
/// }
/// ```
///
/// Neutral custody acceptance remains on the write-store provider contract:
///
/// ```compile_fail
/// fn old_acceptance_door(fava: &fava::Fava, event: fava::EventValue) {
///     let _ = fava.accept_event(event);
/// }
/// ```
///
/// Neutral write-route preview remains on routing/publication providers:
///
/// ```compile_fail
/// fn old_preview_door(fava: &fava::Fava, intent: &fava_write::WriteIntent) {
///     let _ = fava.preview_write_routes(intent);
/// }
/// ```
///
/// Terminal waiting belongs to the returned [`Write`]:
///
/// ```compile_fail
/// async fn old_terminal_wait(fava: &fava::Fava, receipt: fava::ReceiptId) {
///     let _ = fava.wait_terminal(receipt).await;
/// }
/// ```
#[derive(Clone)]
pub struct Fava {
    observer: Observer,
    write_store: Arc<dyn WriteStore>,
    diagnostics: Arc<Diagnostics>,
    routers: Vec<Arc<dyn Router>>,
    session: Session,
    publication: Option<Publication>,
}

impl Fava {
    /// Begin explicit provider assembly.
    #[must_use]
    pub fn builder() -> FavaBuilder {
        FavaBuilder::default()
    }

    /// Open a live query. The returned handle already contains local state.
    ///
    /// # Errors
    ///
    /// Returns [`ObserveError`] when the declarative query is invalid or the
    /// configured local sources cannot establish one coherent initial view.
    #[allow(
        clippy::unused_async,
        reason = "opening is total and synchronous; the async signature is the public door and never awaits a provider"
    )]
    #[allow(
        clippy::result_large_err,
        reason = "ObserveError names the exact source role that refused; a live-relay role carries its session identity"
    )]
    pub async fn observe(&self, query: Query) -> Result<Observation, ObserveError> {
        self.observer.open(query)
    }

    /// Cancel one accepted event before publication work exists.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when cancellation cannot commit atomically.
    pub fn cancel_write(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError> {
        self.write_store
            .cancel(receipt_id)
            .map(|receipt| receipt.is_some())
    }

    /// Durably accept one checked payload and begin its publication lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`PublishError`] when this assembly cannot validate or accept it.
    #[allow(
        clippy::result_large_err,
        reason = "PublishError intentionally carries the complete terminal Receipt as evidence"
    )]
    #[allow(private_bounds)]
    pub fn publish<P>(&self, payload: P) -> Result<Write, PublishError>
    where
        P: publication::PublishPayload,
    {
        publication::publish(self.publication.as_ref(), payload)
    }

    /// Narrow one edit publication to this exact author.
    pub fn by(&self, author: PublicKey) -> PublishAs<'_> {
        publication::by(self, author)
    }

    /// Narrow one publication to an exact bounded relay sequence.
    ///
    /// # Errors
    ///
    /// Returns [`PublishError`] when the normalized route is empty or exceeds
    /// the explicit publication bound.
    #[allow(
        clippy::result_large_err,
        reason = "PublishError intentionally carries the complete terminal Receipt as evidence"
    )]
    pub fn to(
        &self,
        relays: impl IntoIterator<Item = RelayUrl>,
    ) -> Result<PublishTo<'_>, PublishError> {
        publication::to(self, relays)
    }

    /// Read current exact receipt facts.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] when publication is absent or storage fails.
    pub fn receipt(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, PublicationError> {
        self.publication
            .as_ref()
            .ok_or(PublicationError::NotConfigured)?
            .receipt(receipt_id)
    }

    /// Subscribe to committed receipt changes without current-state coalescing.
    ///
    /// Each item pairs its receipt id with the committed current receipt, or
    /// `None` after removal. A slow reader receives an explicit broadcast lag
    /// error rather than silent loss.
    #[must_use]
    pub fn receipt_changes(&self) -> broadcast::Receiver<(ReceiptId, Option<Receipt>)> {
        self.write_store.receipt_changes()
    }

    /// Inspect every currently open publication obligation in receipt order.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when durable current state cannot be read.
    pub fn open_receipts(&self) -> Result<Vec<Receipt>, WriteStoreError> {
        self.write_store.recover_open()
    }

    /// Cancel while every selected destination is definitely pre-handoff.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] when cancellation is unavailable or ineligible.
    pub fn cancel_publication(
        &self,
        receipt_id: ReceiptId,
    ) -> Result<Option<Receipt>, PublicationError> {
        self.publication
            .as_ref()
            .ok_or(PublicationError::NotConfigured)?
            .cancel(receipt_id)
    }

    /// Remove one retained terminal receipt independently of cancellation.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] for active work or storage failure.
    pub fn remove_receipt(&self, receipt_id: ReceiptId) -> Result<bool, PublicationError> {
        self.publication
            .as_ref()
            .ok_or(PublicationError::NotConfigured)?
            .remove_receipt(receipt_id)
    }

    /// Return one bounded immutable snapshot of current exact diagnostic facts.
    #[must_use]
    pub fn diagnostics(&self) -> DiagnosticsSnapshot {
        self.diagnostics.snapshot()
    }

    /// Evaluate current routing facts without opening router or relay work.
    ///
    /// # Errors
    ///
    /// Returns [`ObserveError`] when a configured router refuses preview.
    #[allow(
        clippy::result_large_err,
        reason = "ObserveError names the exact source role that refused; a live-relay role carries its session identity"
    )]
    pub fn preview_routes(&self, query: &Query) -> Result<RoutePlan, ObserveError> {
        let request = RouteRequest::Read(query.clone());
        match query.source().acquisition() {
            fava_query::QueryAcquisition::Explicit(relays) => {
                RoutePlan::explicit(relays.iter().cloned(), query.access(), &request.targets())
                    .map_err(|error| ObserveError::Relay(error.to_string()))
            }
            fava_query::QueryAcquisition::Automatic => {
                fava_routing::preview(&self.routers, &request)
                    .map_err(|error| ObserveError::Relay(error.to_string()))
            }
        }
    }
}

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
