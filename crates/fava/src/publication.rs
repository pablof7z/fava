#![allow(
    clippy::result_large_err,
    reason = "PublishError intentionally carries the complete terminal Receipt as evidence"
)]

//! Application-facing synchronous publication vocabulary.

use std::fmt;

use fava_publication::{Publication, PublicationError};
use fava_write::{
    AuthoredEventBuilder, Event, EventBuilder, EventEdit, PublicKey, Receipt, ReceiptId,
    RelayDeliveryOutcome, UnsignedEvent, WriteId, WriteIntent, WriteIntentError, WriteRouting,
};
use nostr::types::RelayUrl;
use thiserror::Error;

/// One accepted publication obligation as seen by an application.
#[derive(Clone)]
pub struct Write {
    id: WriteId,
    receipt_id: ReceiptId,
    publication: Publication,
}

impl Write {
    /// Stable identity of this accepted publication obligation.
    #[must_use]
    pub const fn write_id(&self) -> WriteId {
        self.id
    }

    /// Stable reattachable identity of this publication receipt.
    #[must_use]
    pub const fn receipt_id(&self) -> ReceiptId {
        self.receipt_id
    }

    /// Read the current complete receipt from its durable owner.
    ///
    /// # Errors
    ///
    /// Returns [`PublishError`] when storage fails or the receipt disappeared.
    pub fn receipt(&self) -> Result<Receipt, PublishError> {
        self.publication
            .receipt(self.receipt_id)?
            .ok_or_else(|| PublicationError::ReceiptMissing(self.receipt_id).into())
    }

    /// Await caller-selected delivery sufficiency or bounded terminality.
    ///
    /// # Errors
    ///
    /// Returns [`PublishError::NotReached`] with the complete terminal receipt
    /// when terminality arrives before the predicate succeeds. Storage and
    /// receipt-stream failures remain attributable through [`PublishError`].
    pub async fn settled<F>(&self, predicate: F) -> Result<Receipt, PublishError>
    where
        F: Fn(&Receipt) -> bool,
    {
        let (receipt, reached) = self
            .publication
            .wait_until(self.receipt_id, predicate)
            .await?;
        if reached {
            Ok(receipt)
        } else {
            Err(PublishError::NotReached { receipt })
        }
    }
}

impl fmt::Debug for Write {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Write")
            .field("write_id", &self.id)
            .field("receipt_id", &self.receipt_id)
            .finish_non_exhaustive()
    }
}

/// An inert, edit-only signer scope for one publication expression.
///
/// Signer scope cannot publish an unsigned event because that event already
/// carries its author:
///
/// ```compile_fail
/// fn unsigned_is_not_an_edit(
///     fava: &fava::Fava,
///     author: fava::PublicKey,
///     event: fava::UnsignedEvent,
/// ) {
///     let _ = fava.by(author).publish(event);
/// }
/// ```
///
/// A pre-signed event has already used its signer and is likewise excluded:
///
/// ```compile_fail
/// fn signed_event_already_has_a_signer(
///     fava: &fava::Fava,
///     author: fava::PublicKey,
///     event: fava::Event,
/// ) {
///     let _ = fava.by(author).publish(event);
/// }
/// ```
///
/// An authored event body has already settled its identity and is likewise
/// excluded:
///
/// ```compile_fail
/// fn authored_builder_already_has_an_author(
///     fava: &fava::Fava,
///     author: fava::PublicKey,
///     builder: fava::AuthoredEventBuilder,
/// ) {
///     let _ = fava.by(author).publish(builder);
/// }
/// ```
#[must_use = "a signer scope is inert until publish is called"]
pub struct PublishAs<'a> {
    fava: &'a crate::Fava,
    author: PublicKey,
    routing: WriteRouting,
}

impl PublishAs<'_> {
    /// Narrow this edit publication to an exact bounded relay sequence.
    ///
    /// # Arguments
    ///
    /// * `relays` - the exact relay sequence to publish to
    ///
    /// # Errors
    ///
    /// Returns [`PublishError`] when raw input exceeds its bound or the
    /// normalized route is empty or exceeds its distinct-destination bound.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fava::{EventBuilder, Fava, Kind, PublicKey, RelayUrl};
    /// # fn publish_gm(fava: &Fava, author: PublicKey) -> Result<(), Box<dyn std::error::Error>> {
    /// let relay = RelayUrl::parse("wss://relay.example")?;
    /// let builder = EventBuilder::new(Kind::TextNote).content("gm");
    /// let write = fava.by(author).to(vec![relay])?.publish(builder)?;
    /// # let _ = write;
    /// # Ok(())
    /// # }
    /// ```
    pub fn to(mut self, relays: impl Into<Vec<RelayUrl>>) -> Result<Self, PublishError> {
        self.routing = explicit_routing(relays)?;
        Ok(self)
    }

    /// Durably accept one authorless payload with this exact author and
    /// routing scope.
    ///
    /// # Arguments
    ///
    /// * `payload` - the authorless event builder or edit to accept
    ///
    /// # Errors
    ///
    /// Returns [`PublishError`] when the payload or publication is refused.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fava::{EventBuilder, Fava, Kind, PublicKey};
    /// # fn publish_gm(fava: &Fava, author: PublicKey) -> Result<(), Box<dyn std::error::Error>> {
    /// let builder = EventBuilder::new(Kind::TextNote).content("gm");
    /// let write = fava.by(author).publish(builder)?;
    /// # let _ = write;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(private_bounds)]
    pub fn publish<P>(self, payload: P) -> Result<Write, PublishError>
    where
        P: AuthorlessPayload,
    {
        publish_scoped(
            self.fava.publication.as_ref(),
            payload,
            Some(self.author),
            self.routing,
        )
    }
}

/// An inert explicit-relay scope for one publication expression.
#[must_use = "a relay scope is inert until publish is called"]
pub struct PublishTo<'a> {
    fava: &'a crate::Fava,
    routing: WriteRouting,
}

impl<'a> PublishTo<'a> {
    /// Add an exact edit author while preserving this explicit route.
    ///
    /// # Arguments
    ///
    /// * `author` - the key that signs the authorless payload
    ///
    /// # Examples
    ///
    /// ```
    /// # use fava::{EventBuilder, Fava, Kind, PublicKey, RelayUrl};
    /// # fn publish_gm(fava: &Fava, author: PublicKey) -> Result<(), Box<dyn std::error::Error>> {
    /// let relay = RelayUrl::parse("wss://relay.example")?;
    /// let builder = EventBuilder::new(Kind::TextNote).content("gm");
    /// let write = fava.to(vec![relay])?.by(author).publish(builder)?;
    /// # let _ = write;
    /// # Ok(())
    /// # }
    /// ```
    pub fn by(self, author: PublicKey) -> PublishAs<'a> {
        PublishAs {
            fava: self.fava,
            author,
            routing: self.routing,
        }
    }

    /// Durably accept one checked payload through this explicit route.
    ///
    /// # Arguments
    ///
    /// * `payload` - the event, edit, or authored builder to accept
    ///
    /// # Errors
    ///
    /// Returns [`PublishError`] when the payload or publication is refused.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fava::{EventBuilder, Fava, Kind, PublicKey, RelayUrl};
    /// # fn publish_gm(fava: &Fava, author: PublicKey) -> Result<(), Box<dyn std::error::Error>> {
    /// let relay = RelayUrl::parse("wss://relay.example")?;
    /// let builder = EventBuilder::new(Kind::TextNote).content("gm").by(author);
    /// let write = fava.to(vec![relay])?.publish(builder)?;
    /// # let _ = write;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(private_bounds)]
    pub fn publish<P>(self, payload: P) -> Result<Write, PublishError>
    where
        P: PublishPayload,
    {
        publish_scoped(self.fava.publication.as_ref(), payload, None, self.routing)
    }
}

/// Refusal at the application publication door.
#[allow(
    clippy::large_enum_variant,
    reason = "NotReached must expose the complete terminal Receipt without indirection"
)]
#[derive(Debug, Error)]
pub enum PublishError {
    /// An authorless edit has no selected current account or explicit signer scope.
    #[error("replaceable edit publication requires an author selection")]
    MissingAuthor,
    /// A zero acknowledgement threshold cannot express delivery sufficiency.
    #[error("settlement acknowledgement threshold must be positive")]
    InvalidSettlementThreshold,
    /// The receipt became terminal before the selected predicate succeeded.
    #[error("publication became terminal before settlement predicate was reached")]
    NotReached {
        /// Complete terminal receipt, including every destination fact.
        receipt: Receipt,
    },
    /// Payload validation refused before durable custody.
    #[error(transparent)]
    Intent(#[from] WriteIntentError),
    /// The neutral publication owner refused or failed.
    #[error(transparent)]
    Publication(#[from] PublicationError),
}

/// Match a receipt only after routing has settled and every currently desired
/// destination has an exact terminal fact.
///
/// Terminal facts include acknowledgement, rejection, exhausted delivery, and
/// ambiguous handoff. This predicate proves completion, not delivery success.
pub fn all_terminal() -> impl Fn(&Receipt) -> bool + Copy {
    |receipt| {
        receipt.route_settled
            && receipt.desired_destinations.iter().all(|session| {
                receipt
                    .destinations()
                    .get(session)
                    .is_some_and(RelayDeliveryOutcome::is_terminal)
            })
    }
}

/// Match a receipt only after routing has settled and every currently desired
/// destination has exact relay acknowledgement evidence.
///
/// A settled route with no destination is not acknowledged. Historical facts
/// for destinations withdrawn from the current route do not satisfy this
/// predicate.
pub fn all_acknowledged() -> impl Fn(&Receipt) -> bool + Copy {
    |receipt| {
        receipt.route_settled
            && !receipt.desired_destinations.is_empty()
            && receipt.desired_destinations.iter().all(|session| {
                matches!(
                    receipt.destinations().get(session),
                    Some(RelayDeliveryOutcome::Acknowledged { .. })
                )
            })
    }
}

/// Match a receipt after at least `minimum` relay acknowledgements.
///
/// # Errors
///
/// Returns [`PublishError::InvalidSettlementThreshold`] when `minimum` is zero.
pub fn at_least(minimum: usize) -> Result<impl Fn(&Receipt) -> bool + Copy, PublishError> {
    if minimum == 0 {
        return Err(PublishError::InvalidSettlementThreshold);
    }
    Ok(move |receipt: &Receipt| receipt.acknowledged() >= minimum)
}

/// Anything an application can hand to `Fava::publish`, converted once into a
/// validated write intent.
pub(crate) trait PublishPayload {
    fn into_intent(
        self,
        author: Option<PublicKey>,
        routing: WriteRouting,
    ) -> Result<WriteIntent, PublishError>;
}

/// Payloads that carry no author of their own and so accept one from an
/// author scope. Implemented only by [`EventBuilder`] and [`EventEdit`] —
/// this is what excludes [`AuthoredEventBuilder`], [`UnsignedEvent`], and
/// [`Event`] from [`PublishAs::publish`].
pub(crate) trait AuthorlessPayload: PublishPayload {}

impl AuthorlessPayload for EventBuilder {}
impl AuthorlessPayload for EventEdit {}

impl PublishPayload for UnsignedEvent {
    fn into_intent(
        self,
        _author: Option<PublicKey>,
        routing: WriteRouting,
    ) -> Result<WriteIntent, PublishError> {
        Ok(WriteIntent::event(self, routing)?)
    }
}

/// Merge a builder's own accumulated routing with the publication
/// expression's facade routing: an explicit route on either side wins over
/// automatic, and two explicit routes conflict.
///
/// Shared by every builder's [`PublishPayload`] impl — the merge rule does
/// not depend on whether the builder already carries an author.
fn merge_builder_routing(
    builder_routing: WriteRouting,
    facade_routing: WriteRouting,
) -> Result<WriteRouting, PublishError> {
    if matches!(&builder_routing, WriteRouting::Explicit(_))
        && matches!(&facade_routing, WriteRouting::Explicit(_))
    {
        return Err(WriteIntentError::ConflictingExplicitRoutes.into());
    }
    Ok(match builder_routing {
        WriteRouting::Automatic => facade_routing,
        explicit @ WriteRouting::Explicit(_) => explicit,
    })
}

impl PublishPayload for EventBuilder {
    fn into_intent(
        self,
        author: Option<PublicKey>,
        facade_routing: WriteRouting,
    ) -> Result<WriteIntent, PublishError> {
        let author = author.ok_or(PublishError::MissingAuthor)?;
        let (event, builder_routing) = self
            .by(author)
            .into_event_and_routing()
            .map_err(WriteIntentError::from)?;
        let routing = merge_builder_routing(builder_routing, facade_routing)?;
        Ok(WriteIntent::event(event, routing)?)
    }
}

impl PublishPayload for AuthoredEventBuilder {
    fn into_intent(
        self,
        _author: Option<PublicKey>,
        facade_routing: WriteRouting,
    ) -> Result<WriteIntent, PublishError> {
        let (event, builder_routing) = self
            .into_event_and_routing()
            .map_err(WriteIntentError::from)?;
        let routing = merge_builder_routing(builder_routing, facade_routing)?;
        Ok(WriteIntent::event(event, routing)?)
    }
}

impl PublishPayload for Event {
    fn into_intent(
        self,
        _author: Option<PublicKey>,
        routing: WriteRouting,
    ) -> Result<WriteIntent, PublishError> {
        Ok(WriteIntent::presigned(self, routing)?)
    }
}

impl PublishPayload for EventEdit {
    fn into_intent(
        self,
        author: Option<PublicKey>,
        routing: WriteRouting,
    ) -> Result<WriteIntent, PublishError> {
        let author = author.ok_or(PublishError::MissingAuthor)?;
        Ok(WriteIntent::edit_as(self, author, routing)?)
    }
}

pub(crate) fn publish<P>(
    publication: Option<&Publication>,
    payload: P,
) -> Result<Write, PublishError>
where
    P: PublishPayload,
{
    publish_scoped(publication, payload, None, WriteRouting::Automatic)
}

pub(crate) fn by(fava: &crate::Fava, author: PublicKey) -> PublishAs<'_> {
    PublishAs {
        fava,
        author,
        routing: WriteRouting::Automatic,
    }
}

pub(crate) fn to(
    fava: &crate::Fava,
    relays: impl Into<Vec<RelayUrl>>,
) -> Result<PublishTo<'_>, PublishError> {
    Ok(PublishTo {
        fava,
        routing: explicit_routing(relays)?,
    })
}

fn explicit_routing(relays: impl Into<Vec<RelayUrl>>) -> Result<WriteRouting, PublishError> {
    Ok(WriteRouting::explicit(relays)?)
}

fn publish_scoped<P>(
    publication: Option<&Publication>,
    payload: P,
    author: Option<PublicKey>,
    routing: WriteRouting,
) -> Result<Write, PublishError>
where
    P: PublishPayload,
{
    let intent = payload.into_intent(author, routing)?;
    let publication = publication.ok_or(PublicationError::NotConfigured)?;
    let accepted = publication.accept(intent)?;
    Ok(Write {
        id: accepted.write_id,
        receipt_id: accepted.receipt_id,
        publication: publication.clone(),
    })
}
