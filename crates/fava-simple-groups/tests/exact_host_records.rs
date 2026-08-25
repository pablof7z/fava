//! Exact selected-relay state-query proof through the public API.

use fava_query::{QueryAcquisition, ResultAuthority};
use fava_simple_groups::{SimpleGroup, SimpleGroupStateEventKind};
use nostr::types::RelayUrl;
use std::collections::BTreeSet;

#[test]
fn state_query_carries_every_configured_relay_as_exact_authority()
-> Result<(), Box<dyn std::error::Error>> {
    let b = RelayUrl::parse("wss://b.groups.example")?;
    let a = RelayUrl::parse("wss://a.groups.example")?;
    let group = SimpleGroup::from_relays("photos", vec![b.clone(), a.clone()])?;
    let query = group.meta_events([SimpleGroupStateEventKind::Metadata])?;
    let relays = BTreeSet::from([b, a]);
    assert_eq!(
        query.source().acquisition(),
        &QueryAcquisition::Explicit(relays.clone())
    );
    assert_eq!(
        query.source().authority(),
        &ResultAuthority::OnlyRelays(relays)
    );
    assert_eq!(query.result_limit(), None);
    Ok(())
}

#[test]
fn state_query_preserves_all_256_exact_relays() -> Result<(), Box<dyn std::error::Error>> {
    let relays = (0..256)
        .map(|index| RelayUrl::parse(&format!("wss://host-{index}.groups.example")))
        .collect::<Result<Vec<_>, _>>()?;
    let expected = relays.iter().cloned().collect::<BTreeSet<_>>();
    let group = SimpleGroup::from_relays("photos", relays)?;
    let query = group.meta_events([SimpleGroupStateEventKind::Metadata])?;
    assert_eq!(
        query.source().acquisition(),
        &QueryAcquisition::Explicit(expected.clone())
    );
    assert_eq!(
        query.source().authority(),
        &ResultAuthority::OnlyRelays(expected)
    );
    assert_eq!(query.result_limit(), None);
    Ok(())
}
