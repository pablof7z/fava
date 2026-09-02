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

/// An inert account scope for one publication expression.
///
/// The account names two facts at once: the relay authority this work runs
/// over, and the author of a payload that states none. When the payload
/// already states its author, the account still names the connection — which
/// is how one person's event goes out over another's authenticated session:
///
/// ```
/// # use fava::{EventBuilder, Fava, Kind, PublicKey};
/// # fn bob_over_alices_connection(
/// #     fava: &Fava,
/// #     alice: PublicKey,
/// #     bob: PublicKey,
/// # ) -> Result<(), Box<dyn std::error::Error>> {
/// let note = EventBuilder::new(Kind::TextNote).content("gm").by(bob);
/// let write = fava.with_account(alice).publish(note)?;
/// # let _ = write;
/// # Ok(())
/// # }
/// ```
///
/// The event is Bob's and the relay session is Alice's. An authorless payload
/// takes the account for both.
#[must_use = "a signer scope is inert until publish is called"]
pub struct PublishAs<'a> {
    fava: &'a crate::Fava,
    /// The account this work runs as: the relay-session authority it goes
    /// over, and the author of a payload that states none.
    account: PublicKey,
    routing: WriteRouting,
}

impl PublishAs<'_> {
    /// Open a live query under this account.
    ///
    /// The selection names the relay authority the read runs over, which is the
    /// same fact it names for a write. An application therefore takes one door
    /// for both.
    ///
    /// # Arguments
    ///
    /// * `query` - the declarative query to open
    ///
    /// # Errors
    ///
    /// Returns [`crate::ObserveError`] when the query is invalid or the
    /// configured local sources cannot establish one coherent initial view.
    pub async fn observe(
        self,
        query: fava_query::Query,
    ) -> Result<crate::Observation, crate::ObserveError> {
        self.fava
            .observe(query.with_relay_access(fava_relay::Authority::As(self.account)))
            .await
    }

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
    /// let write = fava.with_account(author).to(vec![relay])?.publish(builder)?;
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
    /// let write = fava.with_account(author).publish(builder)?;
    /// # let _ = write;
    /// # Ok(())
    /// # }
    /// ```
    pub fn publish<P>(self, payload: P) -> Result<Write, PublishError>
    where
        P: PublishPayload,
    {
        publish_scoped(
            self.fava.publication.as_ref(),
            payload,
            Some(self.account),
            self.routing,
            fava_relay::Authority::As(self.account),
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
    /// let write = fava.to(vec![relay])?.with_account(author).publish(builder)?;
    /// # let _ = write;
    /// # Ok(())
    /// # }
    /// ```
    /// Name the account this work runs as, through this explicit route.
    ///
    /// # Arguments
    ///
    /// * `account` - the account whose authority this work runs under
    pub fn with_account(self, account: PublicKey) -> PublishAs<'a> {
        PublishAs {
            fava: self.fava,
            account,
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
        publish_scoped(
            self.fava.publication.as_ref(),
            payload,
            self.fava.session.current_account(),
            self.routing,
            // No account was named, so this is public work. The current account
            // may author it; it does not make the connection authenticated.
            fava_relay::Authority::Unauthenticated,
        )
    }
}

/// Refusal at the application publication door.
#[allow(
    clippy::large_enum_variant,
    reason = "NotReached must expose the complete terminal Receipt without indirection"
)]
#[derive(Debug, Error)]
pub enum PublishError {
    /// An authorless payload has no selected current account or explicit author scope.
    #[error("authorless publication requires a current account selection or explicit author scope")]
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
/// What a publication expression accepts.
///
/// Implemented for every payload shape: one that states its own author keeps
/// it, and one that states none takes the selected account.
pub trait PublishPayload {
    fn into_intent(
        self,
        author: Option<PublicKey>,
        routing: WriteRouting,
    ) -> Result<WriteIntent, PublishError>;
}

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
    current_account: Option<PublicKey>,
) -> Result<Write, PublishError>
where
    P: PublishPayload,
{
    publish_scoped(
        publication,
        payload,
        current_account,
        WriteRouting::Automatic,
        fava_relay::Authority::Unauthenticated,
    )
}

pub(crate) fn with_account(fava: &crate::Fava, account: PublicKey) -> PublishAs<'_> {
    PublishAs {
        fava,
        account,
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
    access: fava_relay::Authority,
) -> Result<Write, PublishError>
where
    P: PublishPayload,
{
    let intent = payload.into_intent(author, routing)?.under(access);
    let publication = publication.ok_or(PublicationError::NotConfigured)?;
    let accepted = publication.accept(intent)?;
    Ok(Write {
        id: accepted.write_id,
        receipt_id: accepted.receipt_id,
        publication: publication.clone(),
    })
}
