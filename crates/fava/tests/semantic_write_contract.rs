//! Public neutral-contract evidence for semantic replaceable-event writes.

use std::collections::BTreeMap;

use fava_write::{
    EventBuilder, EventValue, Kind, RevisionId, PublicationEvidence, ReceiptId,
    EventEdit, EditApplier, SignatureState, Tag, Timestamp,
    UnsignedEvent, WriteId, WriteIntent, WriteIntentError, WritePayload, WriteRouting,
};
use nostr::key::Keys;

fn edit(kind: Kind, identifier: Option<String>) -> EventEdit {
    EventEdit::new(kind, identifier, vec![1, 2, 3]).expect("bounded edit")
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
    let decoded: EventEdit = serde_json::from_str(&encoded).expect("edit round-trips");
    assert_eq!(decoded, original);

    for foreign in ["actor", "format", "inverse"] {
        let encoded = encoded.replacen('{', &format!("{{\"{foreign}\":0,"), 1);
        assert!(serde_json::from_str::<EventEdit>(&encoded).is_err());
    }
    assert!(serde_json::from_str::<EventEdit>("{malformed").is_err());

    assert_eq!(
        EventEdit::new(Kind::ContactList, None, vec![0; 131_073]),
        Err(WriteIntentError::TooLarge {
            bytes: 131_073,
            maximum: 131_072,
        })
    );
}

struct ExactApplier {
    tag_count: usize,
}

impl EditApplier for ExactApplier {
    fn kind(&self) -> Kind {
        Kind::ContactList
    }

    fn supports(&self, edit: &EventEdit) -> bool {
        edit.kind() == Kind::ContactList && edit.identifier().is_none()
    }

    fn apply(
        &self,
        _edit: &EventEdit,
        author: fava_write::PublicKey,
        source: Option<&EventValue>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        assert!(source.is_none());
        EventBuilder::new(Kind::ContactList)
            .created_at(created_at)
            .content("first value")
            .tags((0..self.tag_count).map(|index| {
                Tag::parse(["x", &index.to_string()]).expect("ordinary applier tag")
            }))
            .by(author)
            .build()
            .map_err(WriteIntentError::from)
    }
}

#[test]
fn first_value_receives_no_prior_and_exact_timestamp() {
    let actor = Keys::generate().public_key();
    let edit = edit(Kind::ContactList, None);
    let timestamp = Timestamp::from(42);
    let applier = ExactApplier { tag_count: 0 };

    assert!(applier.supports(&edit));
    let event = applier
        .apply(&edit, actor, None, timestamp)
        .expect("first value applies");
    assert_eq!(event.pubkey, actor);
    assert_eq!(event.created_at, timestamp);
}

#[test]
fn exact_applier_preserves_the_event_builder_tag_refusal() {
    let actor = Keys::generate().public_key();
    let applier = ExactApplier { tag_count: 2_001 };

    assert_eq!(
        applier.apply(
            &edit(Kind::ContactList, None),
            actor,
            None,
            Timestamp::from(42),
        ),
        Err(WriteIntentError::TooManyTags {
            actual: 2_001,
            maximum: 2_000,
        })
    );
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
fn revision_identity_changes_but_receipt_identity_does_not() {
    let actor = Keys::generate().public_key();
    let write_id = WriteId::try_from(9).expect("nonzero write identity");
    let receipt_id = ReceiptId::try_from(11).expect("nonzero receipt identity");
    let first = RevisionId::FIRST;
    let successor = RevisionId::try_from(2).expect("nonzero revision identity");
    let first_event = EventBuilder::new(Kind::ContactList)
        .created_at(Timestamp::from(1))
        .by(actor)
        .build()
        .expect("first event");
    let successor_event = EventBuilder::new(Kind::ContactList)
        .created_at(Timestamp::from(2))
        .by(actor)
        .build()
        .expect("successor event");
    let first_event_id = first_event.id.expect("first id");
    let successor_event_id = successor_event.id.expect("successor id");
    let evidence = PublicationEvidence {
        receipt_id,
        write_id,
        revision_id: successor,
        revision_source: Some(first_event_id),
        revision_failure: Some("bounded applier refusal".to_owned()),
        retired_revisions: vec![(first, first_event_id, None, None)],
        signature: SignatureState::Unsigned,
        destinations: BTreeMap::new(),
    };

    assert_ne!(first, successor);
    assert_eq!(first.as_u64(), 1);
    assert_eq!(successor.as_u64(), 2);
    assert_eq!((write_id.as_u64(), receipt_id.as_u64()), (9, 11));
    assert_eq!(evidence.write_id, write_id);
    assert_eq!(evidence.receipt_id, receipt_id);
    assert_eq!(evidence.revision_id, successor);
    assert_eq!(evidence.revision_source, Some(first_event_id));
    assert_eq!(evidence.retired_revisions[0].0, first);
    assert_ne!(first_event_id, successor_event_id);
}
