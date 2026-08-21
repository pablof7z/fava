//! External semantic-capability proof compiled outside the Fava workspace.

#[cfg(test)]
mod tests {
    use fava::{
        EventCoordinate, Kind, ReplaceableEventEdit, ReplaceableEventMaterializer, Timestamp,
    };
    use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent, Tag};
    use nostr::key::Keys;

    #[test]
    fn external_first_value_inverse_and_preservation() {
        let keys = Keys::generate();
        let actor = keys.public_key();
        let materializer = selected_materializer();
        let add_alpha = insert(actor, "alpha").expect("bounded external edit");

        let first = materializer
            .materialize(&add_alpha, None, Timestamp::from(10))
            .expect("empty state materializes");
        assert_eq!(first.pubkey, actor);
        assert_eq!(first.kind, external_kind());
        assert_eq!(first.content, "external-set-v1\nalpha\n");

        let preserved_tag = Tag::parse(["x-future", "opaque"]).expect("unknown tag");
        let source = NostrEventBuilder::new(
            external_kind(),
            "external-set-v1\nbeta\nunrelated\ncontent",
        )
        .tag(preserved_tag.clone())
        .custom_created_at(Timestamp::from(20))
        .finalize(&keys)
        .expect("source signs");
        let successor = materializer
            .materialize(&add_alpha, Some(&source), Timestamp::from(21))
            .expect("current state materializes");
        assert_eq!(successor.content, "external-set-v1\nalpha,beta\nunrelated\ncontent");
        assert_eq!(successor.tags, vec![preserved_tag]);

        let successor = successor.finalize(&keys).expect("successor signs");
        let inverse = add_alpha.inverse();
        let restored = materializer
            .materialize(&inverse, Some(&successor), Timestamp::from(22))
            .expect("inverse materializes through the same contract");
        assert_eq!(restored.content, "external-set-v1\nbeta\nunrelated\ncontent");
    }

    #[test]
    fn external_duplicate_adjacent_and_ordering_are_deterministic() {
        let keys = Keys::generate();
        let actor = keys.public_key();
        let materializer = selected_materializer();
        let add_alpha = insert(actor, "alpha").expect("alpha edit");
        let add_beta = insert(actor, "beta").expect("beta edit");

        let beta = materializer
            .materialize(&add_beta, None, Timestamp::from(1))
            .unwrap()
            .finalize(&keys)
            .unwrap();
        let alpha_then_beta = materializer
            .materialize(&add_alpha, Some(&beta), Timestamp::from(2))
            .unwrap();
        let duplicate = materializer
            .materialize(
                &add_alpha,
                Some(&alpha_then_beta.clone().finalize(&keys).unwrap()),
                Timestamp::from(3),
            )
            .unwrap();

        let alpha = materializer
            .materialize(&add_alpha, None, Timestamp::from(1))
            .unwrap()
            .finalize(&keys)
            .unwrap();
        let beta_then_alpha = materializer
            .materialize(&add_beta, Some(&alpha), Timestamp::from(2))
            .unwrap();

        assert_eq!(alpha_then_beta.content, "external-set-v1\nalpha,beta\n");
        assert_eq!(duplicate.content, alpha_then_beta.content);
        assert_eq!(beta_then_alpha.content, alpha_then_beta.content);

        let remove_beta = remove(actor, "beta").expect("remove edit");
        let adjacent = materializer
            .materialize(
                &remove_beta,
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

        assert!(insert(actor, &"x".repeat(257)).is_err());
        let malformed_edit = ReplaceableEventEdit::new(
            actor,
            EventCoordinate::Replaceable {
                author: actor,
                kind: external_kind(),
                identifier: None,
            },
            1,
            vec![99, 0, 0],
            vec![1, 0, 0],
        )
        .unwrap();
        assert!(
            materializer
                .materialize(&malformed_edit, None, Timestamp::from(1))
                .is_err()
        );

        let malformed_source = NostrEventBuilder::new(
            external_kind(),
            "external-set-v1\nnot valid!\nopaque",
        )
        .finalize(&keys)
        .unwrap();
        assert!(
            materializer
                .materialize(
                    &insert(actor, "alpha").unwrap(),
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
                    &insert(actor, "alpha").unwrap(),
                    Some(&oversized_source),
                    Timestamp::from(2),
                )
                .is_err()
        );

        let coordinate = EventCoordinate::Replaceable {
            author: actor,
            kind: Kind::Custom(15_001),
            identifier: None,
        };
        assert_eq!(insert(actor, "alpha").unwrap().coordinate(), &coordinate);
    }
}
