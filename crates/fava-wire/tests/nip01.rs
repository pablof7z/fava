//! NIP-01 wire-shape evidence.

use fava_wire::{ClientMessage, RelayMessage, decode_relay, encode_client};
use nostr::filter::Filter;
use nostr::message::SubscriptionId;

#[test]
fn exact_nip01_req_close_event_eose_and_closed_shapes_round_trip() {
    let id = SubscriptionId::new("m2-exact");
    let req = ClientMessage::req(id.clone(), Filter::new().limit(3));
    assert_eq!(
        encode_client(&req).expect("REQ encodes"),
        r#"["REQ","m2-exact",{"limit":3}]"#
    );
    assert_eq!(
        encode_client(&ClientMessage::close(id.clone())).expect("CLOSE encodes"),
        r#"["CLOSE","m2-exact"]"#
    );
    assert!(matches!(
        decode_relay(r#"["EOSE","m2-exact"]"#).expect("EOSE decodes"),
        RelayMessage::EndOfStoredEvents(found) if found.as_ref() == &id
    ));
    assert!(matches!(
        decode_relay(r#"["CLOSED","m2-exact","rate-limited"]"#).expect("CLOSED decodes"),
        RelayMessage::Closed { subscription_id, message }
            if subscription_id.as_ref() == &id && message.as_ref() == "rate-limited"
    ));
}
