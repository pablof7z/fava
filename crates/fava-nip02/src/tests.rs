use fava_write::{ReplaceableEventEdit, Tag, Timestamp, WriteIntentError};
use nostr::event::{EventBuilder, FinalizeEvent};
use nostr::key::{Keys, PublicKey};

use crate::materializer;

mod contact_list;
mod edit;
mod materializer;
mod query;

fn tag(values: &[&str]) -> Tag {
    Tag::parse(values.iter().copied()).expect("nonempty test tag")
}

fn source(
    keys: &Keys,
    kind: fava_write::Kind,
    created_at: u64,
    content: &str,
    tags: Vec<Tag>,
) -> fava_write::Event {
    EventBuilder::new(kind, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("source signs")
}

fn materialize(
    author: PublicKey,
    edit: &ReplaceableEventEdit,
    source: Option<&fava_write::Event>,
    created_at: u64,
) -> Result<fava_write::UnsignedEvent, WriteIntentError> {
    materializer().materialize(edit, author, source, Timestamp::from(created_at))
}

fn target_tags(event: &fava_write::UnsignedEvent, target: PublicKey) -> usize {
    event
        .tags
        .iter()
        .filter(|tag| {
            let values = tag.as_slice();
            values.first().map(String::as_str) == Some("p")
                && values
                    .get(1)
                    .and_then(|value| PublicKey::from_hex(value).ok())
                    == Some(target)
        })
        .count()
}
