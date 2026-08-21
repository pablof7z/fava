//! External semantic-capability proof compiled outside the Fava workspace.

mod capability;

pub use capability::{
    decode_external_event, external_kind, external_query, insert, remove, selected_materializer,
    validate_external_event,
};

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use fava::{EventValue, Kind, Query, ReplaceableEventEdit, Timestamp, WriteIntentError};
    use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent, Tag};
    use nostr::key::Keys;

    #[test]
    fn external_first_value_opposing_operation_and_preservation() {
        let keys = Keys::generate();
        let actor = keys.public_key();
        let materializer = selected_materializer();
        let add_alpha = insert("alpha").expect("bounded external edit");

        let first = materializer
            .materialize(&add_alpha, actor, None, Timestamp::from(10))
            .expect("empty state materializes");
        assert_eq!(first.pubkey, actor);
        assert_eq!(first.kind, external_kind());
        assert_eq!(first.content, "external-set-v1\nalpha\n");
        let first_value = EventValue::Unsigned(first.clone());
        validate_external_event(&first_value).expect("typed validation accepts first value");
        assert_eq!(
            decode_external_event(&first_value).expect("typed decode accepts first value"),
            (BTreeSet::from(["alpha".to_owned()]), String::new())
        );

        let preserved_tag = Tag::parse(["x-future", "opaque"]).expect("unknown tag");
        let source =
            NostrEventBuilder::new(external_kind(), "external-set-v1\nbeta\nunrelated\ncontent")
                .tag(preserved_tag.clone())
                .custom_created_at(Timestamp::from(20))
                .finalize(&keys)
                .expect("source signs");
        let source_value = EventValue::Signed(source.clone());
        validate_external_event(&source_value).expect("typed validation accepts current source");
        assert_eq!(
            decode_external_event(&source_value).expect("typed decode accepts current source"),
            (
                BTreeSet::from(["beta".to_owned()]),
                "unrelated\ncontent".to_owned()
            )
        );
        let successor = materializer
            .materialize(&add_alpha, actor, Some(&source), Timestamp::from(21))
            .expect("current state materializes");
        assert_eq!(
            successor.content,
            "external-set-v1\nalpha,beta\nunrelated\ncontent"
        );
        assert_eq!(successor.tags.as_slice(), &[preserved_tag]);

        let successor = successor.finalize(&keys).expect("successor signs");
        let remove_alpha = remove("alpha").expect("opposing edit");
        let restored = materializer
            .materialize(&remove_alpha, actor, Some(&successor), Timestamp::from(22))
            .expect("opposing edit materializes through the same contract");
        assert_eq!(
            restored.content,
            "external-set-v1\nbeta\nunrelated\ncontent"
        );
    }

    #[test]
    fn external_duplicate_adjacent_and_ordering_are_deterministic() {
        let keys = Keys::generate();
        let actor = keys.public_key();
        let materializer = selected_materializer();
        let add_alpha = insert("alpha").expect("alpha edit");
        let add_beta = insert("beta").expect("beta edit");

        let beta = materializer
            .materialize(&add_beta, actor, None, Timestamp::from(1))
            .unwrap()
            .finalize(&keys)
            .unwrap();
        let alpha_then_beta = materializer
            .materialize(&add_alpha, actor, Some(&beta), Timestamp::from(2))
            .unwrap();
        let duplicate = materializer
            .materialize(
                &add_alpha,
                actor,
                Some(&alpha_then_beta.clone().finalize(&keys).unwrap()),
                Timestamp::from(3),
            )
            .unwrap();

        let alpha = materializer
            .materialize(&add_alpha, actor, None, Timestamp::from(1))
            .unwrap()
            .finalize(&keys)
            .unwrap();
        let beta_then_alpha = materializer
            .materialize(&add_beta, actor, Some(&alpha), Timestamp::from(2))
            .unwrap();

        assert_eq!(alpha_then_beta.content, "external-set-v1\nalpha,beta\n");
        assert_eq!(duplicate.content, alpha_then_beta.content);
        assert_eq!(beta_then_alpha.content, alpha_then_beta.content);

        let remove_beta = remove("beta").expect("remove edit");
        let adjacent = materializer
            .materialize(
                &remove_beta,
                actor,
                Some(&beta_then_alpha.finalize(&keys).unwrap()),
                Timestamp::from(4),
            )
            .unwrap();
        assert_eq!(adjacent.content, "external-set-v1\nalpha\n");
    }

    #[test]
    fn external_bounds_and_malformed_source_refuse() {
        let keys = Keys::generate();
        let actor = keys.public_key();
        let materializer = selected_materializer();

        assert!(insert(&"x".repeat(257)).is_err());
        let malformed_edit =
            ReplaceableEventEdit::new(external_kind(), None, vec![99, 0, 0]).unwrap();
        assert!(
            materializer
                .materialize(&malformed_edit, actor, None, Timestamp::from(1))
                .is_err()
        );

        let malformed_source =
            NostrEventBuilder::new(external_kind(), "external-set-v1\nnot valid!\nopaque")
                .finalize(&keys)
                .unwrap();
        assert!(
            materializer
                .materialize(
                    &insert("alpha").unwrap(),
                    actor,
                    Some(&malformed_source),
                    Timestamp::from(2),
                )
                .is_err()
        );

        let oversized_source = NostrEventBuilder::new(external_kind(), "z".repeat(4_097))
            .finalize(&keys)
            .unwrap();
        assert!(
            materializer
                .materialize(
                    &insert("alpha").unwrap(),
                    actor,
                    Some(&oversized_source),
                    Timestamp::from(2),
                )
                .is_err()
        );

        let mut too_many_tags_builder = NostrEventBuilder::new(external_kind(), "opaque");
        for index in 0..65 {
            too_many_tags_builder = too_many_tags_builder
                .tag(Tag::parse(vec!["x".to_owned(), index.to_string()]).expect("bounded tag"));
        }
        let too_many_tags = too_many_tags_builder.finalize(&keys).expect("source signs");
        assert!(matches!(
            materializer.materialize(
                &insert("alpha").unwrap(),
                actor,
                Some(&too_many_tags),
                Timestamp::from(3),
            ),
            Err(WriteIntentError::InvalidEvent(message)) if message.contains("tag count")
        ));

        let nested_values = std::iter::once("x".to_owned())
            .chain((0..16).map(|index| index.to_string()))
            .collect::<Vec<_>>();
        let nested_source = NostrEventBuilder::new(external_kind(), "opaque")
            .tag(Tag::parse(nested_values).expect("nested tag"))
            .finalize(&keys)
            .expect("source signs");
        assert!(matches!(
            materializer.materialize(
                &insert("alpha").unwrap(),
                actor,
                Some(&nested_source),
                Timestamp::from(4),
            ),
            Err(WriteIntentError::InvalidEvent(message)) if message.contains("nested values")
        ));

        let large_tag_value = "v".repeat(4_096);
        let tag_heavy_source = NostrEventBuilder::new(external_kind(), "opaque")
            .tag(Tag::parse(["x", large_tag_value.as_str()]).expect("large opaque tag"))
            .finalize(&keys)
            .expect("source signs");
        assert!(matches!(
            materializer.materialize(
                &insert("alpha").unwrap(),
                actor,
                Some(&tag_heavy_source),
                Timestamp::from(5),
            ),
            Err(WriteIntentError::TooLarge { maximum: 4_096, .. })
        ));

        let edit = insert("alpha").unwrap();
        assert_eq!(edit.kind(), Kind::Custom(15_001));
        assert_eq!(edit.identifier(), None);
        assert_eq!(
            external_query(actor),
            Query::events().authors([actor]).kind(external_kind())
        );
    }
}
