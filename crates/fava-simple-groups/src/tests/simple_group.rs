use fava_query::{Kind, Query, QueryAcquisition, ResultAuthority, SingleLetterTag};
use fava_write::{EventBuilder, Timestamp, WriteIntentError, WriteRouting};
use nostr::types::RelayUrl;

use crate::{SimpleGroup, SimpleGroupConstructionError, SimpleGroupStateEventKind};

use super::{public_key, tag};

#[test]
fn construction_rejects_exactly_empty_ids_and_empty_relay_vectors() {
    let relay = RelayUrl::parse("wss://relay.example").expect("relay");

    assert_eq!(
        SimpleGroup::new("", vec![relay]),
        Err(SimpleGroupConstructionError::EmptyId)
    );
    assert_eq!(
        SimpleGroup::new("photos", Vec::new()),
        Err(SimpleGroupConstructionError::EmptyRelays)
    );
}

#[test]
fn construction_preserves_opaque_non_empty_id_and_first_relay_occurrences() {
    let first = RelayUrl::parse("wss://b.example").expect("relay");
    let second = RelayUrl::parse("wss://a.example").expect("relay");
    let group = SimpleGroup::new(" ", vec![first.clone(), second, first])
        .expect("non-empty opaque id and relay selection");

    assert_eq!(group.id(), " ");
    assert_eq!(
        group
            .relays()
            .map(|relay| relay.to_string())
            .collect::<Vec<_>>(),
        ["wss://b.example", "wss://a.example"]
    );
}

#[test]
fn construction_has_no_domain_cap_and_operations_return_owning_errors() {
    let first = RelayUrl::parse("wss://write-0.example").expect("relay");
    let rest: Vec<_> = (1..257)
        .map(|index| {
            RelayUrl::parse(&format!("wss://write-{index}.example")).expect("unique relay")
        })
        .collect();
    let group = SimpleGroup::new("g", std::iter::once(first).chain(rest).collect())
        .expect("non-empty group");
    assert!(matches!(
        WriteRouting::explicit(group.relays()),
        Err(WriteIntentError::TooManyExplicitRelays {
            actual: 257,
            maximum: 256,
        })
    ));
}

#[test]
fn content_query_intersects_the_h_axis_and_uses_ordinary_relay_acquisition() {
    let first = RelayUrl::parse("wss://b.example").expect("relay");
    let second = RelayUrl::parse("wss://a.example").expect("relay");
    let group = SimpleGroup::new("photos", vec![first, second]).expect("non-empty group");
    let selection = Query::events()
        .kinds([Kind::from_u16(9)])
        .expect("one kind is bounded")
        .oldest_first()
        .cache_only()
        .limit(23)
        .expect("positive limit");
    let query = group.events(selection).expect("group content query");
    let h = SingleLetterTag::from_char('h').expect("h tag");
    let axis = |query: &Query| query.selection().tag_values.get(&h).cloned();
    let expected = |values: &[&str]| {
        Some(
            values
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<std::collections::BTreeSet<_>>(),
        )
    };

    assert_eq!(axis(&query), expected(&["photos"]));
    assert!(matches!(
        query.source().acquisition(),
        QueryAcquisition::Explicit(_)
    ));
    assert_eq!(query.source().authority(), &ResultAuthority::AnyLocal);
    assert_eq!(
        query.result_limit().map(std::num::NonZeroUsize::get),
        Some(23)
    );

    let matching = Query::events()
        .tag_values(h, ["photos"])
        .and_then(|selection| group.events(selection))
        .expect("matching group axis");
    assert_eq!(axis(&matching), expected(&["photos"]));

    let disjoint = Query::events()
        .tag_values(h, ["other"])
        .and_then(|selection| group.events(selection))
        .expect("disjoint group axis becomes match-nothing");
    assert_eq!(axis(&disjoint), expected(&[]));

    let narrowed = Query::events()
        .tag_values(h, ["photos", "other"])
        .and_then(|selection| group.events(selection))
        .expect("group axis narrows");
    assert_eq!(axis(&narrowed), expected(&["photos"]));

    let present_empty = Query::events()
        .tag_values(h, std::iter::empty::<String>())
        .and_then(|selection| group.events(selection))
        .expect("present-empty group axis remains match-nothing");
    assert_eq!(axis(&present_empty), expected(&[]));
}

#[test]
fn state_event_query_delegates_empty_and_non_empty_sets_to_query() {
    let relay = RelayUrl::parse("wss://relay.example").expect("relay");
    let group = SimpleGroup::new("photos", vec![relay.clone()]).expect("non-empty group");
    let query = group
        .meta_events([
            SimpleGroupStateEventKind::Pins,
            SimpleGroupStateEventKind::Metadata,
            SimpleGroupStateEventKind::Pins,
        ])
        .expect("state query");
    let d = SingleLetterTag::from_char('d').expect("d tag");

    assert_eq!(
        query.selection().kinds,
        Some(std::collections::BTreeSet::from([
            Kind::from_u16(39_000),
            Kind::from_u16(39_005),
        ]))
    );
    assert_eq!(
        query.selection().tag_values.get(&d),
        Some(&std::collections::BTreeSet::from(["photos".to_owned()]))
    );
    assert_eq!(
        query.source().authority(),
        &ResultAuthority::OnlyRelays(std::collections::BTreeSet::from([relay]))
    );
    assert_eq!(query.result_limit(), None);
    let empty = group
        .meta_events([])
        .expect("the query owner defines empty kinds as match-nothing");
    assert_eq!(
        empty.selection().kinds,
        Some(std::collections::BTreeSet::new())
    );
}

#[test]
fn prepare_preserves_every_existing_tag_and_adds_only_a_missing_match() {
    let relay = RelayUrl::parse("wss://relay.example").expect("relay");
    let group = SimpleGroup::new("photos", vec![relay]).expect("non-empty group");
    let draft = EventBuilder::new(public_key(), Kind::from_u16(42))
        .created_at(Timestamp::from(2))
        .tags([
            tag(&["h"]),
            tag(&["h", "other", "unused"]),
            tag(&["x", "kept"]),
        ])
        .content("opaque")
        .build()
        .expect("draft");
    let prepared = group.prepare(draft).expect("preparation");

    assert_eq!(prepared.tags.len(), 4);
    assert_eq!(prepared.tags[0], tag(&["h"]));
    assert_eq!(prepared.tags[1], tag(&["h", "other", "unused"]));
    assert_eq!(prepared.tags[2], tag(&["x", "kept"]));
    assert_eq!(prepared.tags[3], tag(&["h", "photos"]));
    assert_eq!(group.prepare(prepared.clone()).unwrap(), prepared);
}
