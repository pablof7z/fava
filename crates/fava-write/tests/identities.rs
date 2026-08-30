//! Nonzero reconstruction and checked-generation proofs for durable write identities.

use std::num::NonZeroU64;

use fava_write::{ReceiptId, RevisionId, WriteId};

#[test]
fn durable_write_identities_are_nonzero_and_round_trip_exactly() {
    assert!(WriteId::try_from(0).is_err());
    assert!(ReceiptId::try_from(0).is_err());
    assert!(RevisionId::try_from(0).is_err());

    let raw = 17;
    let write = WriteId::try_from(raw).expect("nonzero write identity");
    let receipt = ReceiptId::try_from(raw).expect("nonzero receipt identity");
    let revision = RevisionId::try_from(raw).expect("nonzero revision identity");

    assert_eq!(write.as_u64(), raw);
    assert_eq!(receipt.as_u64(), raw);
    assert_eq!(revision.as_u64(), raw);
    assert_eq!(
        serde_json::from_str::<WriteId>(&raw.to_string()).expect("write identity decodes"),
        write
    );
    assert_eq!(
        serde_json::from_str::<ReceiptId>(&raw.to_string()).expect("receipt identity decodes"),
        receipt
    );
    assert_eq!(
        serde_json::from_str::<RevisionId>(&raw.to_string()).expect("revision identity decodes"),
        revision
    );
    assert!(serde_json::from_str::<WriteId>("0").is_err());
    assert!(serde_json::from_str::<ReceiptId>("0").is_err());
    assert!(serde_json::from_str::<RevisionId>("0").is_err());

    let nonzero = NonZeroU64::new(raw).expect("constant is nonzero");
    assert_eq!(WriteId::from_nonzero(nonzero), write);
    assert_eq!(ReceiptId::from_nonzero(nonzero), receipt);
    assert_eq!(RevisionId::from_nonzero(nonzero), revision);
}

#[test]
fn revision_generation_advancement_is_checked() {
    assert_eq!(RevisionId::FIRST.as_u64(), 1);
    assert_eq!(
        RevisionId::FIRST
            .checked_next()
            .expect("second generation")
            .as_u64(),
        2
    );
    assert_eq!(
        RevisionId::try_from(u64::MAX)
            .expect("maximum is nonzero")
            .checked_next(),
        None
    );
}
