//! Exact-host record query proof through the public API.

use fava_query::{QueryAcquisition, ResultAuthority};
use fava_simple_groups::{SimpleGroup, SimpleGroupRecords};
use nostr::types::RelayUrl;
use std::collections::BTreeSet;

#[test]
fn exact_host_records_1_builds_one_query_per_configured_host()
-> Result<(), Box<dyn std::error::Error>> {
    let b = RelayUrl::parse("wss://b.groups.example")?;
    let a = RelayUrl::parse("wss://a.groups.example")?;
    let group = SimpleGroup::on([b.clone(), a.clone()], "photos")?;
    let queries = group.records(SimpleGroupRecords::all())?;
    assert_eq!(queries.len(), 2);
    assert_eq!(
        queries.iter().map(|(host, _)| host).collect::<Vec<_>>(),
        [&b, &a]
    );
    for (host, query) in queries {
        let only = BTreeSet::from([host]);
        assert_eq!(
            query.source().acquisition(),
            &QueryAcquisition::Explicit(only.clone())
        );
        assert_eq!(
            query.source().authority(),
            &ResultAuthority::OnlyRelays(only)
        );
        assert_eq!(
            query.result_limit().map(std::num::NonZeroUsize::get),
            Some(4_096)
        );
    }
    Ok(())
}

#[test]
fn exact_host_records_1_preserves_all_256_exact_hosts() -> Result<(), Box<dyn std::error::Error>> {
    let hosts = (0..256)
        .map(|index| RelayUrl::parse(&format!("wss://host-{index}.groups.example")))
        .collect::<Result<Vec<_>, _>>()?;
    let group = SimpleGroup::on(hosts.clone(), "photos")?;
    let queries = group.records(SimpleGroupRecords::all())?;
    assert_eq!(queries.len(), 256);
    for ((host, query), expected) in queries.into_iter().zip(hosts) {
        assert_eq!(host, expected);
        let only = BTreeSet::from([expected]);
        assert_eq!(
            query.source().acquisition(),
            &QueryAcquisition::Explicit(only.clone())
        );
        assert_eq!(
            query.source().authority(),
            &ResultAuthority::OnlyRelays(only)
        );
        assert_eq!(
            query.result_limit().map(std::num::NonZeroUsize::get),
            Some(4_096)
        );
    }
    Ok(())
}
