use std::collections::BTreeSet;

use fava_query::{
    Freshness, Kind, PublicKey, Query, QueryAcquisition, QueryOrdering, RelayUrl, ResultAuthority,
    SingleLetterTag,
};

use crate::{SimpleGroup, SimpleGroupError, SimpleGroupRecords, SimpleGroups};

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("test relay URL")
}

fn simple_group() -> SimpleGroup {
    SimpleGroup::on(
        [relay("wss://z.example"), relay("wss://a.example")],
        "photos",
    )
    .expect("bounded group")
}

#[test]
fn content_query_refuses_unbounded_and_oversized_results() {
    let simple_group = simple_group();

    assert!(matches!(
        simple_group.events(Query::events()),
        Err(SimpleGroupError::Query(_))
    ));
    assert!(matches!(
        simple_group.events(Query::events().limit(4_097).expect("positive limit")),
        Err(SimpleGroupError::Query(_))
    ));
    assert_eq!(
        simple_group
            .events(Query::events().limit(1).expect("positive limit"))
            .expect("minimum result bound")
            .result_limit()
            .expect("retained result bound")
            .get(),
        1
    );
    assert_eq!(
        simple_group
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
    let simple_group = simple_group();
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
    let content = simple_group
        .events(selection)
        .expect("bounded content query");

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
    let simple_group = simple_group();
    let h = SingleLetterTag::from_char('h').expect("tag key");
    let bounded = || Query::events().limit(16).expect("positive limit");

    assert_eq!(
        simple_group.events(bounded().tag_values(h, ["photos"])),
        Err(SimpleGroupError::ConflictingSimpleGroupContext),
        "the capability alone owns insertion of the exact h axis"
    );
    assert_eq!(
        simple_group.events(bounded().tag_values(h, ["photos", "elsewhere"])),
        Err(SimpleGroupError::ConflictingSimpleGroupContext)
    );
    assert_eq!(
        simple_group.events(bounded().tag_values(h, ["elsewhere"])),
        Err(SimpleGroupError::ConflictingSimpleGroupContext)
    );
    assert_eq!(
        simple_group.events(bounded().tag_values(h, std::iter::empty::<String>())),
        Err(SimpleGroupError::EmptySimpleGroupContext)
    );
}

#[test]
fn record_query_uses_exact_relay_authority() {
    let simple_group = simple_group();
    let records = simple_group
        .records(SimpleGroupRecords::all())
        .expect("record query");

    assert_eq!(records.len(), 2);
    for (host, query) in records {
        let singleton = BTreeSet::from([host]);
        assert_eq!(
            query.source().acquisition(),
            &QueryAcquisition::Explicit(singleton.clone())
        );
        assert_eq!(
            query.source().authority(),
            &ResultAuthority::OnlyRelays(singleton)
        );
    }
}

#[test]
fn group_queries_have_explicit_result_bounds() {
    let simple_group = simple_group();
    let mut queries = simple_group
        .records(SimpleGroupRecords::all())
        .expect("record query")
        .into_iter()
        .map(|(_, query)| query)
        .collect::<Vec<_>>();
    queries.extend([
        SimpleGroups::saved_simple_groups(Vec::<PublicKey>::new()).expect("saved-group query"),
        SimpleGroups::saved_relays(Vec::<PublicKey>::new()).expect("saved-relay query"),
        SimpleGroups::simple_groups_where_admin(Vec::<PublicKey>::new()).expect("admin query"),
        SimpleGroups::simple_groups_where_member(Vec::<PublicKey>::new()).expect("member query"),
    ]);

    for query in queries {
        assert_eq!(
            query.result_limit().map(std::num::NonZeroUsize::get),
            Some(4_096),
            "every capability-owned query must declare its whole-result bound"
        );
    }
}
