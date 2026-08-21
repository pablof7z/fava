//! Public replaceable-edit coordinate, authorship, serialization, and bound proofs.

use fava_write::{
    Kind, PublicKey, ReplaceableEventEdit, WriteIntent, WriteIntentError, WritePayload,
    WriteRouting,
};
use serde_json::json;

fn author() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
        .expect("fixed public key")
}

fn plain_edit() -> ReplaceableEventEdit {
    ReplaceableEventEdit::new(Kind::ContactList, None, vec![1, 2, 3])
        .expect("plain replaceable edit")
}

#[test]
fn edit_is_exact_coordinate_minus_author_plus_change() {
    let edit = plain_edit();
    assert_eq!(edit.kind(), Kind::ContactList);
    assert_eq!(edit.identifier(), None);
    assert_eq!(edit.change(), &[1, 2, 3]);
    assert_eq!(
        serde_json::to_value(&edit).expect("edit serializes"),
        json!({"kind": 3, "identifier": null, "change": [1, 2, 3]})
    );
}

#[test]
fn addressable_edit_shape_is_valid_and_bounded() {
    let edit =
        ReplaceableEventEdit::new(Kind::from_u16(30_023), Some("article".to_owned()), vec![1])
            .expect("addressable coordinate");
    assert_eq!(edit.identifier(), Some("article"));
    assert!(matches!(
        ReplaceableEventEdit::new(Kind::from_u16(30_023), Some("x".repeat(4_097)), vec![1],),
        Err(WriteIntentError::TooLarge {
            bytes: 4_097,
            maximum: 4_096
        })
    ));
    assert!(
        ReplaceableEventEdit::new(
            Kind::ContactList,
            Some("not-addressable".to_owned()),
            vec![1],
        )
        .is_err()
    );
    assert!(ReplaceableEventEdit::new(Kind::from_u16(30_023), None, vec![1]).is_err());
}

#[test]
fn accepted_edit_payload_carries_the_resolved_author() {
    let author = author();
    let edit = plain_edit();
    let intent = WriteIntent::edit_as(edit.clone(), author, WriteRouting::Automatic)
        .expect("explicit edit author");
    assert_eq!(intent.author(), author);
    assert!(matches!(
        intent.payload(),
        WritePayload::Edit {
            edit: stored,
            author: stored_author
        } if stored == &edit && *stored_author == author
    ));
    let encoded = serde_json::to_vec(&intent).expect("intent serializes");
    let recovered: WriteIntent = serde_json::from_slice(&encoded).expect("intent recovers");
    assert_eq!(recovered, intent);
}

#[test]
fn superseded_edit_json_fields_are_refused() {
    assert!(serde_json::from_value::<ReplaceableEventEdit>(json!({
        "kind": 3,
        "change": [1]
    }))
    .is_err());
    for obsolete in ["actor", "coordinate", "format", "inverse"] {
        let mut value = serde_json::to_value(plain_edit()).expect("edit serializes");
        value
            .as_object_mut()
            .expect("object")
            .insert(obsolete.to_owned(), json!(1));
        assert!(
            serde_json::from_value::<ReplaceableEventEdit>(value).is_err(),
            "obsolete {obsolete} must be refused"
        );
    }
}
