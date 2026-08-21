//! Compile-and-run proof that raw event construction needs only the `fava` dependency.

use fava::{EventBuildError, EventBuilder, Kind, PublicKey, Tag, Timestamp};

fn main() {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .expect("fixed public key");
    let tags = vec![
        Tag::parse(["something something"]).expect("arbitrary flag tag"),
        Tag::parse(["a", "poop"]).expect("arbitrary value tag"),
        Tag::parse(["x-future", "kept", "verbatim"]).expect("future tag"),
    ];
    let event = EventBuilder::from_parts(
        author,
        Kind::Custom(50_001),
        Timestamp::from(123),
        tags.clone(),
        "boobs".to_owned(),
    )
    .build()
    .expect("arbitrary raw event builds");
    assert_eq!(event.pubkey, author);
    assert_eq!(event.kind, Kind::Custom(50_001));
    assert_eq!(event.created_at, Timestamp::from(123));
    assert_eq!(event.tags.as_slice(), tags.as_slice());
    assert_eq!(event.content, "boobs");

    let excessive_tags =
        (0..=2_000).map(|index| Tag::parse(["x", &index.to_string()]).expect("bounded tag"));
    match EventBuilder::new(author, Kind::Custom(50_002))
        .tags(excessive_tags)
        .build()
    {
        Err(EventBuildError::TooManyTags {
            actual: 2_001,
            maximum: 2_000,
        }) => {}
        other => panic!("unexpected oversized builder result: {other:?}"),
    }
}
