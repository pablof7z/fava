use std::collections::BTreeMap;

use fava_write::{Event, Receipt, ReceiptOutcome, RelayDeliveryOutcome, WriteRouting};
use nostr::types::RelayUrl;

pub(super) fn destinations(routing: &WriteRouting) -> BTreeMap<RelayUrl, RelayDeliveryOutcome> {
    match routing {
        WriteRouting::Automatic => BTreeMap::new(),
        WriteRouting::Explicit(relays) => relays
            .iter()
            .cloned()
            .map(|relay| (relay, RelayDeliveryOutcome::Pending))
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

#[cfg(test)]
mod tests {
    use fava_write::WriteRouting;
    use nostr::types::RelayUrl;

    use super::destinations;

    #[test]
    fn ordered_route_derives_one_lane_per_identity() {
        let first = relay("first");
        let second = relay("second");
        let routing = WriteRouting::explicit([first.clone(), second.clone(), first])
            .expect("route normalizes");

        assert_eq!(
            routing,
            WriteRouting::Explicit(vec![relay("first"), relay("second")])
        );
        assert_eq!(destinations(&routing).len(), 2);
    }

    fn relay(name: &str) -> RelayUrl {
        RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
    }
}
