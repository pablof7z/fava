use std::collections::BTreeSet;

use fava_query::{
    Freshness, Kind, PublicKey, Query, QueryAcquisition, QueryOrdering, RelayUrl, ResultAuthority,
    SingleLetterTag,
};

use crate::{Group, GroupError, GroupRecords, SimpleGroups};

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("test relay URL")
}

fn group() -> Group {
    Group::on(
        [relay("wss://z.example"), relay("wss://a.example")],
        "photos",
    )
    .expect("bounded group")
}

#[test]
fn content_query_refuses_unbounded_and_oversized_results() {
    let group = group();

    assert!(matches!(
        group.events(Query::events()),
        Err(GroupError::Query(_))
    ));
    assert!(matches!(
        group.events(Query::events().limit(4_097).expect("positive limit")),
        Err(GroupError::Query(_))
    ));
    assert_eq!(
        group
            .events(Query::events().limit(1).expect("positive limit"))
            .expect("minimum result bound")
            .result_limit()
            .expect("retained result bound")
            .get(),
        1
    );
    assert_eq!(
        group
            .events(Query::events().limit(4_096).expect("positive limit"))
            .expect("maximum result bound")
            .result_limit()
            .expect("retained result bound")
            .get(),
        4_096
    );
}

#[test]
fn content_query_preserves_any_local_visibility() {
    let group = group();
    let hosts = BTreeSet::from([relay("wss://z.example"), relay("wss://a.example")]);
    let h = SingleLetterTag::from_char('h').expect("tag key");
    let selection = Query::events()
        .kind(Kind::from_u16(9))
        .tag_values(
            SingleLetterTag::from_char('p').expect("tag key"),
            ["subject"],
        )
        .limit(87)
        .expect("positive limit")
        .cache_only()
        .oldest_first();
    let content = group.events(selection).expect("bounded content query");

    assert_eq!(
        content.selection().kinds,
        Some(BTreeSet::from([Kind::from_u16(9)]))
    );
    assert_eq!(
        content.selection().tag_values.get(&h),
        Some(&BTreeSet::from(["photos".to_owned()]))
    );
    assert_eq!(
        content
            .selection()
            .tag_values
            .get(&SingleLetterTag::from_char('p').expect("tag key")),
        Some(&BTreeSet::from(["subject".to_owned()]))
    );
    assert_eq!(
        content.source().acquisition(),
        &QueryAcquisition::Explicit(hosts)
    );
    assert_eq!(content.source().authority(), &ResultAuthority::AnyLocal);
    assert_eq!(content.result_limit().expect("retained limit").get(), 87);
    assert_eq!(content.freshness(), Freshness::CacheOnly);
    assert_eq!(content.ordering(), QueryOrdering::OldestFirst);
}

#[test]
fn content_query_refuses_every_preexisting_group_context() {
    let group = group();
    let h = SingleLetterTag::from_char('h').expect("tag key");
    let bounded = || Query::events().limit(16).expect("positive limit");

    assert_eq!(
        group.events(bounded().tag_values(h, ["photos"])),
        Err(GroupError::ConflictingGroupContext),
        "the capability alone owns insertion of the exact h axis"
    );
    assert_eq!(
        group.events(bounded().tag_values(h, ["photos", "elsewhere"])),
        Err(GroupError::ConflictingGroupContext)
    );
    assert_eq!(
        group.events(bounded().tag_values(h, ["elsewhere"])),
        Err(GroupError::ConflictingGroupContext)
    );
    assert_eq!(
        group.events(bounded().tag_values(h, std::iter::empty::<String>())),
        Err(GroupError::EmptyGroupContext)
    );
}

#[test]
fn record_query_uses_exact_relay_authority() {
    let group = group();
    let hosts = BTreeSet::from([relay("wss://z.example"), relay("wss://a.example")]);
    let records = group.records(GroupRecords::all()).expect("record query");

    assert_eq!(
        records.source().acquisition(),
        &QueryAcquisition::Explicit(hosts.clone())
    );
    assert_eq!(
        records.source().authority(),
        &ResultAuthority::OnlyRelays(hosts)
    );
}

#[test]
fn group_queries_have_explicit_result_bounds() {
    let group = group();
    let queries = [
        group.records(GroupRecords::all()).expect("record query"),
        SimpleGroups::saved_groups(Vec::<PublicKey>::new()).expect("saved-group query"),
        SimpleGroups::saved_relays(Vec::<PublicKey>::new()).expect("saved-relay query"),
        SimpleGroups::groups_where_admin(Vec::<PublicKey>::new()).expect("admin query"),
        SimpleGroups::groups_where_member(Vec::<PublicKey>::new()).expect("member query"),
    ];

    for query in queries {
        assert_eq!(
            query.result_limit().map(std::num::NonZeroUsize::get),
            Some(4_096),
            "every capability-owned query must declare its whole-result bound"
        );
    }
}
