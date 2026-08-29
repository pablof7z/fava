//! Component evidence for deterministic local source merge semantics.

use std::collections::{BTreeMap, BTreeSet};

use fava_query::{
    Query, QueryEvaluator, SingleLetterTag, SourceEvent, SourceKind, SourceRevision,
    SourceSnapshot, SourceStatus,
};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_state::RelayEvent;
use fava_write::{
    EventValue, LocalWriteEvent, PublicationEvidence, ReceiptId, SignatureState, WriteId,
};
use nostr::event::{
    Event, EventBuilder, EventId, FinalizeEvent, FinalizeUnsignedEvent, Kind, Tag, UnsignedEvent,
};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

fn signed_event(keys: &Keys, kind: Kind, created_at: u64, content: &str) -> Event {
    EventBuilder::new(kind, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("test event signs")
}

fn unsigned_event(keys: &Keys, kind: Kind, created_at: u64, content: &str) -> UnsignedEvent {
    let mut event = EventBuilder::new(kind, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize_unsigned(keys.public_key());
    event.ensure_id();
    event
}

fn signed_event_with_tags(keys: &Keys, created_at: u64, content: &str, tags: Vec<Tag>) -> Event {
    EventBuilder::new(Kind::TextNote, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("test event signs")
}

fn unsigned_event_with_tags(
    keys: &Keys,
    created_at: u64,
    content: &str,
    tags: Vec<Tag>,
) -> UnsignedEvent {
    let mut event = EventBuilder::new(Kind::TextNote, content)
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize_unsigned(keys.public_key());
    event.ensure_id();
    event
}

fn literal_tag(key: char, value: &str, later_cells: &[&str]) -> Tag {
    let mut cells = vec![key.to_string(), value.to_owned()];
    cells.extend(later_cells.iter().map(|cell| (*cell).to_owned()));
    Tag::parse(cells).expect("valid literal tag")
}

fn local_unsigned(event: UnsignedEvent) -> SourceEvent {
    SourceEvent::Local(
        LocalWriteEvent::new(
            EventValue::Unsigned(event),
            PublicationEvidence {
                signature: SignatureState::Unsigned,
                ..publication()
            },
        )
        .expect("unsigned event is finalized"),
    )
}

fn result_ids(snapshot: &fava_query::QuerySnapshot) -> BTreeSet<EventId> {
    snapshot
        .events
        .iter()
        .map(fava_query::EventRecord::id)
        .collect()
}

fn relay_source(event: Event, url: &str, observed_at: u64) -> SourceEvent {
    SourceEvent::Relay(RelayEvent::new(
        event,
        RelaySessionKey {
            relay: RelayUrl::parse(url).expect("test relay url"),
            access: RelayAccess::Public,
        },
        Timestamp::from(observed_at),
    ))
}

fn publication() -> PublicationEvidence {
    PublicationEvidence {
        receipt_id: ReceiptId::try_from(7).expect("nonzero receipt identity"),
        write_id: WriteId::try_from(11).expect("nonzero write identity"),
        revision_id: fava_write::RevisionId::FIRST,
        revision_source: None,
        revision_failure: None,
        retired_revisions: Vec::new(),
        signature: SignatureState::Signed,
        destinations: BTreeMap::new(),
    }
}

fn snapshot(kind: SourceKind, events: Vec<SourceEvent>) -> SourceSnapshot {
    SourceSnapshot {
        kind,
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        retractions: Vec::new(),
        events,
    }
}

#[test]
fn same_signed_event_merges_relay_and_publication_evidence() {
    let keys = Keys::generate();
    let event = signed_event(&keys, Kind::TextNote, 10, "hello");
    let local = LocalWriteEvent::new(EventValue::Signed(event.clone()), publication())
        .expect("signed event is valid local state");
    let sources = [
        snapshot(
            SourceKind::EventCache,
            vec![
                relay_source(event.clone(), "wss://relay-a.example", 1),
                relay_source(event, "wss://relay-b.example", 2),
            ],
        ),
        snapshot(SourceKind::WriteStore, vec![SourceEvent::Local(local)]),
    ];
    let query = Query::events();

    let result = StandardQueryEvaluator
        .evaluate(&query, &sources)
        .expect("evaluation succeeds");

    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].relay_occurrences().len(), 2);
    assert_eq!(result.events[0].publication(), Some(&publication()));
}

#[test]
fn local_replacement_overlays_then_reveals_cached_predecessor() {
    let keys = Keys::generate();
    let predecessor = signed_event(&keys, Kind::ContactList, 10, "predecessor");
    let successor = unsigned_event(&keys, Kind::ContactList, 20, "successor");
    let successor_id = successor.id.expect("builder computes id");
    let local = LocalWriteEvent::new(
        EventValue::Unsigned(successor),
        PublicationEvidence {
            signature: SignatureState::Unsigned,
            ..publication()
        },
    )
    .expect("unsigned event is finalized");
    let cache = snapshot(
        SourceKind::EventCache,
        vec![relay_source(predecessor.clone(), "wss://relay.example", 1)],
    );
    let writes = snapshot(SourceKind::WriteStore, vec![SourceEvent::Local(local)]);
    let query = Query::events()
        .kinds([Kind::ContactList])
        .expect("one kind is bounded");

    let overlaid = StandardQueryEvaluator
        .evaluate(&query, &[cache.clone(), writes])
        .expect("evaluation succeeds");
    assert_eq!(overlaid.events.len(), 1);
    assert_eq!(overlaid.events[0].id(), successor_id);

    let after_cancel = StandardQueryEvaluator
        .evaluate(&query, &[cache])
        .expect("evaluation succeeds");
    assert_eq!(after_cancel.events.len(), 1);
    assert_eq!(after_cancel.events[0].id(), predecessor.id);
}

#[test]
fn nonmatching_local_replacement_does_not_shadow_a_selected_predecessor() {
    let keys = Keys::generate();
    let predecessor = EventBuilder::new(Kind::ContactList, "predecessor")
        .tags([literal_tag('d', "wanted", &[])])
        .custom_created_at(Timestamp::from(10_u64))
        .finalize(&keys)
        .expect("test event signs");
    let successor = unsigned_event(&keys, Kind::ContactList, 20, "successor");
    let cache = snapshot(
        SourceKind::EventCache,
        vec![relay_source(predecessor.clone(), "wss://relay.example", 1)],
    );
    let writes = snapshot(SourceKind::WriteStore, vec![local_unsigned(successor)]);
    let query = Query::events()
        .kinds([Kind::ContactList])
        .expect("one kind is bounded")
        .tag_values(
            SingleLetterTag::from_char('d').expect("tag key"),
            ["wanted"],
        )
        .expect("one tag value is bounded");

    let overlaid = StandardQueryEvaluator
        .evaluate(&query, &[cache.clone(), writes])
        .expect("evaluation succeeds");
    assert_eq!(result_ids(&overlaid), BTreeSet::from([predecessor.id]));

    let after_cancel = StandardQueryEvaluator
        .evaluate(&query, &[cache])
        .expect("evaluation succeeds");
    assert_eq!(result_ids(&after_cancel), BTreeSet::from([predecessor.id]));
}

// `local_replacement_without_relay_evidence_shadows_qualified_cached_predecessor` stood
// here and asserted that a write-store-only successor empties an `only_from_relays`
// query. That contradicts `docs/spec/partial-spec-api-semantics.md:200` and `:214`, so it
// is deleted rather than relaxed. The specified behavior is proved by
// `relay_authority.rs::unpublished_local_event_cannot_hide_a_relay_qualified_predecessor`.

#[test]
fn asking_relays_and_trusting_only_relays_are_distinct() {
    let keys = Keys::generate();
    let event = signed_event(&keys, Kind::TextNote, 10, "source authority");
    let asked = RelayUrl::parse("wss://asked.example").expect("relay url");
    let other = "wss://other.example";
    let sources = [snapshot(
        SourceKind::EventCache,
        vec![relay_source(event.clone(), other, 1)],
    )];
    let acquisition_only = Query::events()
        .from_relays([asked.clone()])
        .expect("valid query");
    let provenance_constrained = Query::events()
        .only_from_relays([asked.clone()])
        .expect("valid query");

    let visible = StandardQueryEvaluator
        .evaluate(&acquisition_only, &sources)
        .expect("evaluation succeeds");
    let hidden = StandardQueryEvaluator
        .evaluate(&provenance_constrained, &sources)
        .expect("evaluation succeeds");
    assert_eq!(visible.events.len(), 1);
    assert!(hidden.events.is_empty());

    let qualified_sources = [snapshot(
        SourceKind::EventCache,
        vec![
            relay_source(event.clone(), other, 1),
            relay_source(event, "wss://asked.example", 2),
        ],
    )];
    let now_visible = StandardQueryEvaluator
        .evaluate(&provenance_constrained, &qualified_sources)
        .expect("evaluation succeeds");
    assert_eq!(now_visible.events.len(), 1);
}

#[test]
fn replaceable_tie_selects_the_lowest_event_id() {
    let keys = Keys::generate();
    let left = signed_event(&keys, Kind::Metadata, 10, "left");
    let right = signed_event(&keys, Kind::Metadata, 10, "right");
    let expected = left.id.min(right.id);
    let sources = [snapshot(
        SourceKind::EventCache,
        vec![
            relay_source(left, "wss://relay.example", 1),
            relay_source(right, "wss://relay.example", 2),
        ],
    )];

    let result = StandardQueryEvaluator
        .evaluate(
            &Query::events()
                .kinds([Kind::Metadata])
                .expect("one kind is bounded"),
            &sources,
        )
        .expect("evaluation succeeds");

    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].id(), expected);
}

#[test]
fn equal_timestamp_ordering_and_limit_follow_event_id_contract() {
    let keys = Keys::generate();
    let left = signed_event(&keys, Kind::TextNote, 10, "left");
    let right = signed_event(&keys, Kind::TextNote, 10, "right");
    let least = left.id.min(right.id);
    let greatest = left.id.max(right.id);
    let sources = [snapshot(
        SourceKind::EventCache,
        vec![
            relay_source(left, "wss://relay.example", 1),
            relay_source(right, "wss://relay.example", 2),
        ],
    )];

    let newest = StandardQueryEvaluator
        .evaluate(&Query::events(), &sources)
        .expect("newest-first evaluation succeeds");
    let oldest = StandardQueryEvaluator
        .evaluate(&Query::events().oldest_first(), &sources)
        .expect("oldest-first evaluation succeeds");
    let newest_limited = StandardQueryEvaluator
        .evaluate(
            &Query::events().limit(1).expect("positive result limit"),
            &sources,
        )
        .expect("limited newest-first evaluation succeeds");
    let oldest_limited = StandardQueryEvaluator
        .evaluate(
            &Query::events()
                .oldest_first()
                .limit(1)
                .expect("positive result limit"),
            &sources,
        )
        .expect("limited oldest-first evaluation succeeds");

    assert_eq!(
        newest
            .events
            .iter()
            .map(fava_query::EventRecord::id)
            .collect::<Vec<_>>(),
        vec![greatest, least]
    );
    assert_eq!(
        oldest
            .events
            .iter()
            .map(fava_query::EventRecord::id)
            .collect::<Vec<_>>(),
        vec![least, greatest]
    );
    assert_eq!(newest_limited.events[0].id(), greatest);
    assert_eq!(oldest_limited.events[0].id(), least);
}

#[test]
// Keep the signed/unsigned exact-cell matrix together so one query and one
// source-evidence assertion cover the same semantic counterexamples.
#[allow(clippy::too_many_lines)]
fn literal_tag_selection_matches_exact_signed_and_unsigned_cells() {
    let keys = Keys::generate();
    let signed = signed_event_with_tags(
        &keys,
        10,
        "signed exact",
        vec![
            literal_tag('e', "café", &["ignored"]),
            literal_tag('P', "CaseSensitive", &[]),
        ],
    );
    let unsigned = unsigned_event_with_tags(
        &keys,
        11,
        "unsigned exact",
        vec![
            literal_tag('e', "東京", &[]),
            literal_tag('P', "CaseSensitive", &[]),
        ],
    );
    let unsigned_id = unsigned.id.expect("builder computes id");
    let opposite_key = signed_event_with_tags(
        &keys,
        12,
        "opposite key",
        vec![
            literal_tag('E', "café", &[]),
            literal_tag('P', "CaseSensitive", &[]),
        ],
    );
    let wrong_value_case = unsigned_event_with_tags(
        &keys,
        13,
        "wrong value case",
        vec![
            literal_tag('e', "CAFÉ", &[]),
            literal_tag('P', "CaseSensitive", &[]),
        ],
    );
    let later_cell_only = unsigned_event_with_tags(
        &keys,
        14,
        "later cell decoy",
        vec![
            literal_tag('e', "wrong", &["café"]),
            literal_tag('P', "CaseSensitive", &[]),
        ],
    );
    let missing_conjunct =
        signed_event_with_tags(&keys, 15, "missing P", vec![literal_tag('e', "café", &[])]);
    let all_ids = [
        signed.id,
        unsigned_id,
        opposite_key.id,
        wrong_value_case.id.expect("builder computes id"),
        later_cell_only.id.expect("builder computes id"),
        missing_conjunct.id,
    ];
    let sources = [
        snapshot(
            SourceKind::EventCache,
            vec![
                relay_source(signed.clone(), "wss://relay.example", 1),
                relay_source(opposite_key, "wss://relay.example", 2),
                relay_source(missing_conjunct, "wss://relay.example", 3),
            ],
        ),
        snapshot(
            SourceKind::WriteStore,
            vec![
                local_unsigned(unsigned),
                local_unsigned(wrong_value_case),
                local_unsigned(later_cell_only),
            ],
        ),
    ];
    let query = Query::events()
        .ids(all_ids)
        .and_then(|query| query.authors([keys.public_key()]))
        .expect("test ids and author are bounded")
        .kinds([Kind::TextNote])
        .expect("one kind is bounded")
        .tag_values(
            SingleLetterTag::from_char('e').expect("tag key"),
            ["café", "東京"],
        )
        .and_then(|query| {
            query.tag_values(
                SingleLetterTag::from_char('P').expect("tag key"),
                ["CaseSensitive"],
            )
        })
        .expect("literal tag inputs are bounded");

    let result = StandardQueryEvaluator
        .evaluate(&query, &sources)
        .expect("evaluation succeeds");

    assert_eq!(
        result_ids(&result),
        BTreeSet::from([signed.id, unsigned_id])
    );
    assert_eq!(result.evidence.sources.len(), 2);
    assert_eq!(result.evidence.sources[0].kind, SourceKind::EventCache);
    assert_eq!(result.evidence.sources[1].kind, SourceKind::WriteStore);
}

#[test]
fn all_ascii_letter_keys_select_only_the_exact_case() {
    let keys = Keys::generate();

    for (index, character) in ('a'..='z').chain('A'..='Z').enumerate() {
        let opposite = if character.is_ascii_lowercase() {
            character.to_ascii_uppercase()
        } else {
            character.to_ascii_lowercase()
        };
        let signed_key = if character.is_ascii_lowercase() {
            character
        } else {
            opposite
        };
        let unsigned_key = if character.is_ascii_uppercase() {
            character
        } else {
            opposite
        };
        let signed = signed_event_with_tags(
            &keys,
            index as u64 * 2 + 20,
            "signed key case",
            vec![literal_tag(signed_key, "exact", &[])],
        );
        let unsigned = unsigned_event_with_tags(
            &keys,
            index as u64 * 2 + 21,
            "unsigned key case",
            vec![literal_tag(unsigned_key, "exact", &[])],
        );
        let unsigned_id = unsigned.id.expect("builder computes id");
        let expected = if character.is_ascii_lowercase() {
            signed.id
        } else {
            unsigned_id
        };
        let sources = [
            snapshot(
                SourceKind::EventCache,
                vec![relay_source(signed, "wss://relay.example", 1)],
            ),
            snapshot(SourceKind::WriteStore, vec![local_unsigned(unsigned)]),
        ];
        let query = Query::events()
            .tag_values(
                SingleLetterTag::from_char(character).expect("ASCII tag key"),
                ["exact"],
            )
            .expect("one tag value is bounded");

        let result = StandardQueryEvaluator
            .evaluate(&query, &sources)
            .expect("evaluation succeeds");

        assert_eq!(
            result_ids(&result),
            BTreeSet::from([expected]),
            "tag key {character} must not match {opposite}"
        );
    }
}

#[test]
fn present_empty_literal_tag_axis_matches_nothing() {
    let keys = Keys::generate();
    let signed = signed_event_with_tags(&keys, 10, "signed", vec![literal_tag('e', "exact", &[])]);
    let unsigned =
        unsigned_event_with_tags(&keys, 11, "unsigned", vec![literal_tag('e', "exact", &[])]);
    let sources = [
        snapshot(
            SourceKind::EventCache,
            vec![relay_source(signed, "wss://relay.example", 1)],
        ),
        snapshot(SourceKind::WriteStore, vec![local_unsigned(unsigned)]),
    ];
    let query = Query::events()
        .tag_values(
            SingleLetterTag::from_char('e').expect("tag key"),
            std::iter::empty::<String>(),
        )
        .expect("empty tag input is bounded");

    let result = StandardQueryEvaluator
        .evaluate(&query, &sources)
        .expect("evaluation succeeds");

    assert!(result.events.is_empty());
    assert_eq!(result.evidence.sources.len(), 2);
}
