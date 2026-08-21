use std::collections::BTreeMap;

use fava_state::{RelayAccess, RelaySessionKey};
use fava_write::{Event, Receipt, ReceiptOutcome, RelayDeliveryOutcome, WriteRouting};

pub(super) fn destinations(
    routing: &WriteRouting,
    access: &RelayAccess,
) -> BTreeMap<RelaySessionKey, RelayDeliveryOutcome> {
    match routing {
        WriteRouting::Automatic => BTreeMap::new(),
        WriteRouting::Explicit(relays) => relays
            .iter()
            .cloned()
            .map(|relay| {
                (
                    RelaySessionKey::new(relay, access.clone()),
                    RelayDeliveryOutcome::Pending,
                )
            })
            .collect(),
    }
}

pub(super) fn settle(receipt: &mut Receipt) {
    if receipt.route_settled
        && receipt
            .destinations()
            .values()
            .all(RelayDeliveryOutcome::is_terminal)
    {
        receipt.outcome = if receipt.desired_destinations.is_empty() {
            ReceiptOutcome::NoDestination
        } else {
            ReceiptOutcome::Complete
        };
    }
}

#[derive(Eq, PartialEq)]
pub(super) struct UnsignedEventView<'a> {
    id: Option<fava_write::EventId>,
    pubkey: fava_write::PublicKey,
    created_at: fava_write::Timestamp,
    kind: fava_write::Kind,
    tags: &'a [fava_write::Tag],
    content: &'a str,
}

impl<'a> From<&'a fava_write::UnsignedEvent> for UnsignedEventView<'a> {
    fn from(event: &'a fava_write::UnsignedEvent) -> Self {
        Self {
            id: event.id,
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags.as_slice(),
            content: &event.content,
        }
    }
}

impl<'a> From<&'a Event> for UnsignedEventView<'a> {
    fn from(event: &'a Event) -> Self {
        Self {
            id: Some(event.id),
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags.as_slice(),
            content: &event.content,
        }
    }
}
