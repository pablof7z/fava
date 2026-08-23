//! Attribution of one inbound relay frame to the wire plan installed on it.
//!
//! Attribution resolves against the *installed* set for the current generation,
//! so a frame naming a wire id this generation never accepted is refused rather
//! than admitted. A wire id may serve several logical demands; the owner fans
//! the resulting fact out to every one of them.

use std::collections::BTreeMap;

use fava_event_cache::EventCache;
use fava_query::BoundedText;
use fava_state::{RelaySessionKey, Timestamp};
use fava_subscriptions::InstalledSubscriptions;
use fava_wire::{RelayMessage, SubscriptionId, decode_relay};

/// What one inbound frame turned out to be.
pub(crate) enum Accepted {
    /// Nothing the observation owner acts on.
    Nothing,
    /// One verified event was admitted.
    Event,
    /// The relay has sent everything it stored for one wire subscription.
    StoredEventsComplete(SubscriptionId),
    /// The relay refused one wire subscription.
    Refused {
        /// Wire subscription the relay named.
        id: SubscriptionId,
        /// Verbatim, bounded relay text.
        message: BoundedText,
    },
    /// The relay demands NIP-42 authentication for this session.
    AuthenticationRequired,
    /// The frame could not be attributed to installed demand.
    Unattributed(BoundedText),
}

/// Attribute one inbound frame against the installed plan.
pub(crate) fn accept(
    cache: &dyn EventCache,
    session: &RelaySessionKey,
    installed: &InstalledSubscriptions,
    frame: &[u8],
) -> Accepted {
    let Ok(text) = std::str::from_utf8(frame) else {
        return Accepted::Unattributed(BoundedText::new("relay frame was not valid UTF-8"));
    };
    let message = match decode_relay(text) {
        Ok(message) => message,
        Err(error) => {
            return Accepted::Unattributed(BoundedText::new(format!(
                "invalid relay message: {error}"
            )));
        }
    };
    match message {
        RelayMessage::Event {
            subscription_id,
            event,
        } => {
            let id = subscription_id.into_owned();
            let Some(entry) = installed.get(&id) else {
                return Accepted::Unattributed(BoundedText::new(format!(
                    "unattributed EVENT for {id}"
                )));
            };
            let event = event.into_owned();
            // Admission takes the subscription's whole accepted filter set and
            // authorizes on their union, which is NIP-01 REQ semantics. Calling
            // once per filter would re-derive that union here and reintroduce
            // the narrowing that let a grouped member be checked against a
            // filter its own demand never accepted.
            let accepted = BTreeMap::from([(id.clone(), entry.filters.clone())]);
            match fava_ingest::admit_subscription_event(
                cache,
                session,
                &accepted,
                &id,
                event,
                Timestamp::now(),
            ) {
                Ok(_) => Accepted::Event,
                Err(error) => Accepted::Unattributed(BoundedText::new(error.to_string())),
            }
        }
        RelayMessage::EndOfStoredEvents(subscription) => {
            let id = subscription.into_owned();
            if installed.get(&id).is_some() {
                Accepted::StoredEventsComplete(id)
            } else {
                Accepted::Unattributed(BoundedText::new(format!("unattributed EOSE for {id}")))
            }
        }
        RelayMessage::Closed {
            subscription_id,
            message,
        } => {
            let id = subscription_id.into_owned();
            if installed.get(&id).is_some() {
                Accepted::Refused {
                    id,
                    message: BoundedText::new(message),
                }
            } else {
                Accepted::Unattributed(BoundedText::new(format!("unattributed CLOSED for {id}")))
            }
        }
        RelayMessage::Auth { .. } => Accepted::AuthenticationRequired,
        RelayMessage::Notice(message) => {
            Accepted::Unattributed(BoundedText::new(format!("relay NOTICE: {message}")))
        }
        RelayMessage::Ok { .. }
        | RelayMessage::Count { .. }
        | RelayMessage::NegMsg { .. }
        | RelayMessage::NegErr { .. } => Accepted::Nothing,
    }
}
