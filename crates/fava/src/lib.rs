//! Thin Rust facade over the selected Fava provider assembly.

mod builder;
mod publication;
mod session;

/// Bounded-freshness fetch cache and NIP-05 / NIP-11 service surface.
///
/// [`fetch::FetchCache`] stores HTTP outcomes keyed by URL. Both NIP-05
/// identifier resolution and NIP-11 relay info fetching are available as
/// standalone functions that use this cache rather than live network calls
/// when a fresh entry is already present.
pub mod fetch {
    pub use fava_fetch_cache::{
        FetchAge, FetchCache, FetchOutcome, HttpFetcher, HttpResponse, MemoryFetchCache,
    };
    /// NIP-05 identifier resolution with negative-cache semantics.
    pub mod nip05 {
        pub use fava_fetch_cache::nip05::{Nip05Result, resolve};
    }
    /// NIP-11 relay info fetching with stale-result evidence.
    pub mod nip11 {
        pub use fava_fetch_cache::nip11::{Nip11Result, fetch};
    }
}

use std::sync::Arc;

pub use builder::{BuildError, FavaBuilder};
use fava_diagnostics::Diagnostics;
pub use fava_diagnostics::{
    BoundKind, DiagnosticsSnapshot, DroppedFacts, LimitDiagnostic, LimitScope,
    LogicalDemandDiagnostic, ObservationId, ObservationWireBinding, OperationGeneration,
    ProviderDiagnostic, ProviderKind, ProviderOperation, ProviderOperationState, QueryDiagnostic,
    RelayDiagnostic, RelaySessionState, WireSubscriptionDiagnostic, WriteDiagnostic, WriteStall,
};
use fava_observe::Observer;
pub use fava_observe::{Observation, ObservationClosed, ObserveError};
use fava_publication::Publication;
pub use fava_publication::PublicationError;
pub use fava_query::{
    EventRecord, Freshness, Query, QueryRevision, QuerySnapshot, ResultAuthority, SingleLetterTag,
};
pub use fava_routing::RoutePlan;
pub use fava_runtime::{Runtime, RuntimeConfig};
use fava_session::Session;
pub use fava_session::SessionError;
pub use fava_write::{
    AuthoredEventBuilder, Event, EventBuildError, EventBuilder, EventValue, Kind, RevisionId,
    PublicKey, Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, EventEdit,
    EditApplier, Tag, Timestamp, UnsignedEvent, WriteId, WriteIntentError,
    WriteRouting,
};
use fava_write_store::WriteStore;
pub use fava_write_store::WriteStoreError;
pub use nostr::types::RelayUrl;
pub use publication::{
    PublishAs, PublishError, PublishTo, Write, all_acknowledged, all_terminal, at_least,
};
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

    /// Narrow one publication to an exact bounded finite owned relay sequence.
    ///
    /// # Errors
    ///
    /// Returns [`PublishError`] when raw input exceeds its bound or the
    /// normalized route is empty or exceeds its distinct-destination bound.
    #[allow(
        clippy::result_large_err,
        reason = "PublishError intentionally carries the complete terminal Receipt as evidence"
    )]
    pub fn to(&self, relays: impl Into<Vec<RelayUrl>>) -> Result<PublishTo<'_>, PublishError> {
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
        self.observer.preview_routes(query)
    }
}
