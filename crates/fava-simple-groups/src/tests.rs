mod codec;
mod query;
mod records;
mod saved;
mod simple_group;

use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};

fn public_key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
        .expect("generator public key")
}

fn tag(values: &[&str]) -> Tag {
    Tag::parse(values.iter().copied()).expect("valid tag representation")
}

fn value(kind: u16, tags: Vec<Tag>) -> EventValue {
    EventValue::Unsigned(
        EventBuilder::new(Kind::from_u16(kind))
            .by(public_key())
            .created_at(Timestamp::from(1))
            .tags(tags)
            .build()
            .expect("valid event body"),
    )
}
