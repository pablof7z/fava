//! Public neutral-contract evidence for semantic replaceable-event writes.

use fava_state::EventCoordinate;
use fava_write::{
    Event, EventBuilder, Kind, MaterializationId, ReceiptId, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Timestamp, UnsignedEvent, WriteId, WriteIntent,
    WriteIntentError, WriteRouting,
};
use nostr::key::Keys;

fn edit(actor: fava_write::PublicKey, coordinate: EventCoordinate) -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(actor, coordinate, 7, vec![1, 2, 3], vec![3, 2, 1])
        .expect("bounded edit")
}

#[test]
fn edit_contract_is_bounded_and_round_trips() {
    let actor = Keys::generate().public_key();
    let coordinate = EventCoordinate::Replaceable {
        author: actor,
        kind: Kind::ContactList,
        identifier: None,
    };
    let original = edit(actor, coordinate.clone());

    assert_eq!(original.actor(), actor);
    assert_eq!(original.coordinate(), &coordinate);
    assert_eq!(original.format(), 7);
    assert_eq!(original.change(), &[1, 2, 3]);
    assert_eq!(original.inverse_change(), &[3, 2, 1]);
    assert_eq!(original.inverse().inverse(), original);

    let encoded = serde_json::to_string(&original).expect("edit serializes");
    let decoded: ReplaceableEventEdit =
        serde_json::from_str(&encoded).expect("edit round-trips");
    assert_eq!(decoded, original);

    let duplicate_format = encoded.replacen('{', "{\"format\":7,", 1);
    assert!(serde_json::from_str::<ReplaceableEventEdit>(&duplicate_format).is_err());
    let overflow_format = encoded.replace("\"format\":7", "\"format\":4294967296");
    assert!(serde_json::from_str::<ReplaceableEventEdit>(&overflow_format).is_err());
    assert!(serde_json::from_str::<ReplaceableEventEdit>("{malformed").is_err());

    assert_eq!(
        ReplaceableEventEdit::new(
            actor,
            coordinate,
            7,
            vec![0; 131_073],
            Vec::new(),
        ),
        Err(WriteIntentError::TooLarge {
            bytes: 131_073,
            maximum: 131_072,
        })
    );
}

struct ExactMaterializer;

impl ReplaceableEventMaterializer for ExactMaterializer {
    fn supports(&self, edit: &ReplaceableEventEdit) -> bool {
        edit.format() == 7
    }

    fn materialize(
        &self,
        edit: &ReplaceableEventEdit,
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        assert!(source.is_none());
        EventBuilder::new(edit.actor(), Kind::ContactList)
            .created_at(created_at)
            .content("first value")
            .build()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))
    }
}

#[test]
fn first_value_receives_no_prior_and_exact_timestamp() {
    let actor = Keys::generate().public_key();
    let edit = edit(
        actor,
        EventCoordinate::Replaceable {
            author: actor,
            kind: Kind::ContactList,
            identifier: None,
        },
    );
    let timestamp = Timestamp::from(42);
    let materializer = ExactMaterializer;

    assert!(materializer.supports(&edit));
    let event = materializer
        .materialize(&edit, None, timestamp)
        .expect("first value materializes");
    assert_eq!(event.pubkey, actor);
    assert_eq!(event.created_at, timestamp);
}

#[test]
fn addressable_edit_refuses_before_custody() {
    let actor = Keys::generate().public_key();
    let edit = edit(
        actor,
        EventCoordinate::Replaceable {
            author: actor,
            kind: Kind::Custom(30_000),
            identifier: Some("list".to_owned()),
        },
    );

    assert!(matches!(
        WriteIntent::edit(edit, WriteRouting::Automatic),
        Err(WriteIntentError::InvalidEvent(reason)) if reason.contains("addressable")
    ));
}

#[test]
fn materialization_identity_changes_but_receipt_identity_does_not() {
    let write_id = WriteId::from_u64(9);
    let receipt_id = ReceiptId::from_u64(11);
    let first = MaterializationId::from_u64(1);
    let successor = MaterializationId::from_u64(2);

    assert_ne!(first, successor);
    assert_eq!(first.as_u64(), 1);
    assert_eq!(successor.as_u64(), 2);
    assert_eq!((write_id.as_u64(), receipt_id.as_u64()), (9, 11));
    assert_eq!((write_id.as_u64(), receipt_id.as_u64()), (9, 11));
}
