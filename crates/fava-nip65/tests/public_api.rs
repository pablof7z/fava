//! External compile-surface proof for the NIP-65-owned decoder API.

use fava_nip65::{RelayList, RelayListError};
use fava_write::{EventBuilder, EventValue, Kind, PublicKey};

fn author() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
        .expect("generator public key")
}

#[test]
fn wrong_kind_reports_named_actual_kind_and_exact_text() {
    let event = EventBuilder::new(author(), Kind::TextNote)
        .build()
        .expect("bounded event");
    let error = RelayList::from_event(&EventValue::Unsigned(event)).expect_err("wrong kind");

    assert_eq!(error, RelayListError::WrongKind { actual: 1 });
    assert_eq!(error.to_string(), "expected kind 10002, got 1");
}
