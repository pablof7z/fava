//! External compile and behavior tracer for the simple-groups capability.

use std::collections::BTreeSet;

use fava_query::{Kind, Query, QueryAcquisition, RelayUrl, ResultAuthority, SingleLetterTag};
use fava_write::{EventBuilder, PublicKey, Timestamp};

use fava_simple_groups::{Group, GroupError, GroupRecords};

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("test relay URL")
}

fn group(host: RelayUrl, id: &str) -> Result<Group, GroupError> {
    Group::on([host], id)
}

#[test]
fn one_host_group_traces_pure_preparation_and_queries() {
    let host = relay("wss://groups.example");
    let group_id = " photos ";
    let group = group(host.clone(), group_id).expect("one host is a valid group");
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .expect("generator public key");
    let draft = EventBuilder::new(author, Kind::from_u16(9_007))
        .created_at(Timestamp::from(7))
        .content("opaque content")
        .build()
        .expect("bounded draft");
    let prepared = group.prepare(draft).expect("pure preparation");
    let content = group
        .events(
            Query::events()
                .kind(Kind::from_u16(9))
                .limit(50)
                .expect("positive limit"),
        )
        .expect("ordinary content query");
    let h = SingleLetterTag::from_char('h').expect("tag key");
    let contexts: Vec<_> = prepared
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().map(String::as_str) == Some("h"))
        .map(|tag| tag.as_slice().to_vec())
        .collect();
    let hosts: Vec<_> = group.hosts().collect();

    assert_eq!(group.id(), group_id, "the opaque id must not be trimmed");
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0], host);
    assert_eq!(
        (
            contexts,
            content.selection().tag_values.get(&h),
            content.source().acquisition(),
            content.source().authority(),
        ),
        (
            vec![vec!["h".to_owned(), group_id.to_owned()]],
            Some(&BTreeSet::from([group_id.to_owned()])),
            &QueryAcquisition::Explicit(BTreeSet::from([host])),
            &ResultAuthority::AnyLocal,
        )
    );
}

#[test]
fn group_records_uses_exact_fixed_kind_set() {
    let host = relay("wss://groups.example");
    let group = group(host.clone(), "photos").expect("one host is a valid group");
    let records = group
        .records(GroupRecords::all())
        .expect("ordinary record query");
    let d = SingleLetterTag::from_char('d').expect("tag key");
    let kinds = BTreeSet::from([
        Kind::from_u16(39_000),
        Kind::from_u16(39_001),
        Kind::from_u16(39_002),
        Kind::from_u16(39_003),
        Kind::from_u16(39_004),
        Kind::from_u16(39_005),
    ]);

    assert_eq!(
        (
            records.selection().kinds.as_ref(),
            records.selection().tag_values.get(&d),
            records.source().acquisition(),
            records.source().authority(),
        ),
        (
            Some(&kinds),
            Some(&BTreeSet::from(["photos".to_owned()])),
            &QueryAcquisition::Explicit(BTreeSet::from([host.clone()])),
            &ResultAuthority::OnlyRelays(BTreeSet::from([host])),
        )
    );
}
