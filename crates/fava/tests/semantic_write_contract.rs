//! Public neutral-contract evidence for semantic replaceable-event writes.

use std::collections::BTreeMap;

use fava_write::{
    Event, EventBuilder, Kind, MaterializationId, PublicationEvidence, ReceiptId,
    ReplaceableEventEdit, ReplaceableEventMaterializer, SignatureState, Timestamp, UnsignedEvent,
    WriteId, WriteIntent, WriteIntentError, WritePayload, WriteRouting,
};
use nostr::key::Keys;

fn edit(kind: Kind, identifier: Option<String>) -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(kind, identifier, vec![1, 2, 3]).expect("bounded edit")
}

#[test]
fn edit_contract_is_bounded_and_round_trips() {
    let actor = Keys::generate().public_key();
    let original = edit(Kind::ContactList, None);

    assert_eq!(original.kind(), Kind::ContactList);
    assert_eq!(original.identifier(), None);
    assert_eq!(original.change(), &[1, 2, 3]);
    let intent = WriteIntent::edit_as(original.clone(), actor, WriteRouting::Automatic)
        .expect("structurally valid edit becomes the third write form");
    assert!(matches!(
        intent.payload(),
        WritePayload::Edit { edit, author } if edit == &original && *author == actor
    ));

    let encoded = serde_json::to_string(&original).expect("edit serializes");
    let decoded: ReplaceableEventEdit = serde_json::from_str(&encoded).expect("edit round-trips");
    assert_eq!(decoded, original);

    for foreign in ["actor", "format", "inverse"] {
        let encoded = encoded.replacen('{', &format!("{{\"{foreign}\":0,"), 1);
        assert!(serde_json::from_str::<ReplaceableEventEdit>(&encoded).is_err());
    }
    assert!(serde_json::from_str::<ReplaceableEventEdit>("{malformed").is_err());

    assert_eq!(
        ReplaceableEventEdit::new(Kind::ContactList, None, vec![0; 131_073]),
        Err(WriteIntentError::TooLarge {
            bytes: 131_073,
            maximum: 131_072,
        })
    );
}

struct ExactMaterializer;

impl ReplaceableEventMaterializer for ExactMaterializer {
    fn kind(&self) -> Kind {
        Kind::ContactList
    }

    fn supports(&self, edit: &ReplaceableEventEdit) -> bool {
        edit.kind() == Kind::ContactList && edit.identifier().is_none()
    }

    fn materialize(
        &self,
        _edit: &ReplaceableEventEdit,
        author: fava_write::PublicKey,
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        assert!(source.is_none());
        EventBuilder::new(author, Kind::ContactList)
            .created_at(created_at)
            .content("first value")
            .build()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))
    }
}

#[test]
fn first_value_receives_no_prior_and_exact_timestamp() {
    let actor = Keys::generate().public_key();
    let edit = edit(Kind::ContactList, None);
    let timestamp = Timestamp::from(42);
    let materializer = ExactMaterializer;

    assert!(materializer.supports(&edit));
    let event = materializer
        .materialize(&edit, actor, None, timestamp)
        .expect("first value materializes");
    assert_eq!(event.pubkey, actor);
    assert_eq!(event.created_at, timestamp);
}

#[test]
fn addressable_edit_accepts_an_explicit_author_before_custody() {
    let actor = Keys::generate().public_key();
    let edit = edit(Kind::Custom(30_000), Some("list".to_owned()));

    let intent = WriteIntent::edit_as(edit.clone(), actor, WriteRouting::Automatic)
        .expect("addressable edit validates");
    assert_eq!(edit.identifier(), Some("list"));
    assert_eq!(intent.author(), actor);
}

#[test]
fn materialization_identity_changes_but_receipt_identity_does_not() {
    let actor = Keys::generate().public_key();
    let write_id = WriteId::from_u64(9);
    let receipt_id = ReceiptId::from_u64(11);
    let first = MaterializationId::from_u64(1);
    let successor = MaterializationId::from_u64(2);
    let first_event = EventBuilder::new(actor, Kind::ContactList)
        .created_at(Timestamp::from(1))
        .build()
        .expect("first event");
    let successor_event = EventBuilder::new(actor, Kind::ContactList)
        .created_at(Timestamp::from(2))
        .build()
        .expect("successor event");
    let first_event_id = first_event.id.expect("first id");
    let successor_event_id = successor_event.id.expect("successor id");
    let evidence = PublicationEvidence {
        receipt_id,
        write_id,
        materialization_id: successor,
        materialization_source: Some(first_event_id),
        materialization_failure: Some("bounded materializer refusal".to_owned()),
        retired_materializations: vec![(first, first_event_id, None, None)],
        signature: SignatureState::Unsigned,
        destinations: BTreeMap::new(),
    };

    assert_ne!(first, successor);
    assert_eq!(first.as_u64(), 1);
    assert_eq!(successor.as_u64(), 2);
    assert_eq!((write_id.as_u64(), receipt_id.as_u64()), (9, 11));
    assert_eq!(evidence.write_id, write_id);
    assert_eq!(evidence.receipt_id, receipt_id);
    assert_eq!(evidence.materialization_id, successor);
    assert_eq!(evidence.materialization_source, Some(first_event_id));
    assert_eq!(evidence.retired_materializations[0].0, first);
    assert_ne!(first_event_id, successor_event_id);
}
