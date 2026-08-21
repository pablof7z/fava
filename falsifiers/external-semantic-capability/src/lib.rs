//! External semantic-capability proof compiled outside the Fava workspace.

use std::collections::BTreeSet;
use std::sync::Arc;

use fava::{
    Event, EventBuilder, EventCoordinate, Kind, PublicKey, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Timestamp, UnsignedEvent, WriteIntentError,
};

const KIND: Kind = Kind::Custom(15_001);
const FORMAT: u32 = 1;
const INSERT: u8 = 1;
const REMOVE: u8 = 2;
const MAX_ITEM_BYTES: usize = 256;
const MAX_SOURCE_BYTES: usize = 4_096;
const CONTENT_PREFIX: &str = "external-set-v1\n";

/// Return the unrelated non-addressable replaceable kind used by this proof.
#[must_use]
pub const fn external_kind() -> Kind {
    KIND
}

/// Construct one bounded insertion edit using only public Fava values.
///
/// # Errors
///
/// Returns an existing write-intent refusal when the item is malformed or
/// exceeds the capability's private bound.
pub fn insert(actor: PublicKey, item: &str) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(actor, item, INSERT, REMOVE)
}

/// Construct one bounded removal edit using only public Fava values.
///
/// # Errors
///
/// Returns an existing write-intent refusal when the item is malformed or
/// exceeds the capability's private bound.
pub fn remove(actor: PublicKey, item: &str) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(actor, item, REMOVE, INSERT)
}

/// Return the selected provider behind the public neutral contract.
#[must_use]
pub fn selected_materializer() -> Arc<dyn ReplaceableEventMaterializer> {
    Arc::new(ExternalSetMaterializer)
}

fn edit(
    actor: PublicKey,
    item: &str,
    operation: u8,
    inverse: u8,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    let change = encode_action(operation, item)?;
    let inverse = encode_action(inverse, item)?;
    ReplaceableEventEdit::new(
        actor,
        EventCoordinate::Replaceable {
            author: actor,
            kind: KIND,
            identifier: None,
        },
        FORMAT,
        change,
        inverse,
    )
}

fn encode_action(operation: u8, item: &str) -> Result<Vec<u8>, WriteIntentError> {
    validate_item(item)?;
    let length = u16::try_from(item.len()).map_err(|_| item_refusal())?;
    let mut encoded = Vec::with_capacity(item.len() + 3);
    encoded.push(operation);
    encoded.extend_from_slice(&length.to_be_bytes());
    encoded.extend_from_slice(item.as_bytes());
    Ok(encoded)
}

fn decode_action(bytes: &[u8]) -> Result<(u8, &str), WriteIntentError> {
    let [operation, high, low, item @ ..] = bytes else {
        return Err(edit_refusal());
    };
    if !matches!(*operation, INSERT | REMOVE)
        || usize::from(u16::from_be_bytes([*high, *low])) != item.len()
    {
        return Err(edit_refusal());
    }
    let item = std::str::from_utf8(item).map_err(|_| edit_refusal())?;
    validate_item(item)?;
    Ok((*operation, item))
}

fn validate_pair(edit: &ReplaceableEventEdit) -> Result<(u8, String), WriteIntentError> {
    let (operation, item) = decode_action(edit.change())?;
    let (inverse, inverse_item) = decode_action(edit.inverse_change())?;
    if inverse_item != item || !matches!((operation, inverse), (INSERT, REMOVE) | (REMOVE, INSERT))
    {
        return Err(edit_refusal());
    }
    Ok((operation, item.to_owned()))
}

fn validate_item(item: &str) -> Result<(), WriteIntentError> {
    if item.is_empty()
        || item.len() > MAX_ITEM_BYTES
        || !item
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        Err(item_refusal())
    } else {
        Ok(())
    }
}

fn item_refusal() -> WriteIntentError {
    WriteIntentError::InvalidEvent(
        "external capability item must be 1..=256 lowercase ASCII bytes".to_owned(),
    )
}

fn edit_refusal() -> WriteIntentError {
    WriteIntentError::InvalidEvent("external capability edit encoding is malformed".to_owned())
}

struct ExternalSetMaterializer;

impl ReplaceableEventMaterializer for ExternalSetMaterializer {
    fn kind(&self) -> Kind {
        KIND
    }

    fn supports(&self, edit: &ReplaceableEventEdit) -> bool {
        edit.format() == FORMAT
            && matches!(
                edit.coordinate(),
                EventCoordinate::Replaceable {
                    author,
                    kind,
                    identifier: None,
                } if *author == edit.actor() && *kind == KIND
            )
            && validate_pair(edit).is_ok()
    }

    fn materialize(
        &self,
        edit: &ReplaceableEventEdit,
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        if !self.supports(edit) {
            return Err(edit_refusal());
        }
        if let Some(source) = source
            && (source.pubkey != edit.actor() || source.kind != KIND)
        {
            return Err(WriteIntentError::InvalidEvent(
                "external capability source has the wrong coordinate".to_owned(),
            ));
        }
        let (mut items, preserved) = decode_source(source)?;
        let (operation, item) = validate_pair(edit)?;
        match operation {
            INSERT => {
                items.insert(item);
            }
            REMOVE => {
                items.remove(&item);
            }
            _ => unreachable!("validated operation"),
        }
        let state = items.into_iter().collect::<Vec<_>>().join(",");
        let content = format!("{CONTENT_PREFIX}{state}\n{preserved}");
        if content.len() > MAX_SOURCE_BYTES {
            return Err(WriteIntentError::TooLarge {
                bytes: content.len(),
                maximum: MAX_SOURCE_BYTES,
            });
        }
        let mut builder = EventBuilder::new(edit.actor(), KIND)
            .created_at(created_at)
            .content(content);
        if let Some(source) = source {
            for tag in source.tags.iter().cloned() {
                builder = builder.tag(tag);
            }
        }
        builder
            .build()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))
    }
}

fn decode_source(source: Option<&Event>) -> Result<(BTreeSet<String>, String), WriteIntentError> {
    let Some(source) = source else {
        return Ok((BTreeSet::new(), String::new()));
    };
    if source.content.len() > MAX_SOURCE_BYTES {
        return Err(WriteIntentError::TooLarge {
            bytes: source.content.len(),
            maximum: MAX_SOURCE_BYTES,
        });
    }
    let Some(encoded) = source.content.strip_prefix(CONTENT_PREFIX) else {
        return Ok((BTreeSet::new(), source.content.clone()));
    };
    let (state, preserved) = encoded.split_once('\n').ok_or_else(edit_refusal)?;
    let mut items = BTreeSet::new();
    if !state.is_empty() {
        for item in state.split(',') {
            validate_item(item)?;
            if !items.insert(item.to_owned()) {
                return Err(WriteIntentError::InvalidEvent(
                    "external capability source contains duplicate state".to_owned(),
                ));
            }
        }
    }
    Ok((items, preserved.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fava::{EventCoordinate, Kind, ReplaceableEventEdit, Timestamp};
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
        let source =
            NostrEventBuilder::new(external_kind(), "external-set-v1\nbeta\nunrelated\ncontent")
                .tag(preserved_tag.clone())
                .custom_created_at(Timestamp::from(20))
                .finalize(&keys)
                .expect("source signs");
        let successor = materializer
            .materialize(&add_alpha, Some(&source), Timestamp::from(21))
            .expect("current state materializes");
        assert_eq!(
            successor.content,
            "external-set-v1\nalpha,beta\nunrelated\ncontent"
        );
        assert_eq!(successor.tags.as_slice(), &[preserved_tag]);

        let successor = successor.finalize(&keys).expect("successor signs");
        let inverse = add_alpha.inverse();
        let restored = materializer
            .materialize(&inverse, Some(&successor), Timestamp::from(22))
            .expect("inverse materializes through the same contract");
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

        let malformed_source =
            NostrEventBuilder::new(external_kind(), "external-set-v1\nnot valid!\nopaque")
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
