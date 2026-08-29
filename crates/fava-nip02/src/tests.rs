use fava_write::{EventValue, EventEdit, Tag, Timestamp, WriteIntentError};
use nostr::event::{EventBuilder, FinalizeEvent};
use nostr::key::{Keys, PublicKey};

use crate::applier;

mod contact_list;
mod edit;
mod applier;
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

fn apply(
    author: PublicKey,
    edit: &EventEdit,
    source: Option<&fava_write::Event>,
    created_at: u64,
) -> Result<fava_write::UnsignedEvent, WriteIntentError> {
    let source = source.cloned().map(EventValue::Signed);
    applier().apply(edit, author, source.as_ref(), Timestamp::from(created_at))
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
