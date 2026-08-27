//! Public Fava and `fava-simple-groups` flow across two controlled Croissant relays.
//!
//! This cohesive executable scenario owns the exact cross-relay facts retained by the canary.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fava::{EventBuilder, Fava, Kind, Observation, Query, Receipt, RelayUrl, Tag, Timestamp};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer::Signer;
use fava_signer_local::LocalSigner;
use fava_simple_groups::{
    SimpleGroup, SimpleGroupAdmins, SimpleGroupEventBuilder, SimpleGroupMetadata,
    SimpleGroupStateEventKind,
};
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write::{Event, ReceiptOutcome};
use fava_write_store_redb::RedbWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::croissant::CroissantReadyFact;
use crate::croissant_simple_groups_wire::{
    exact_event_handoffs, verify_query_completion, wait_for_query_completion,
};
use crate::publication_support::wait_terminal;
use crate::{CanaryError, CanaryResult, WireProxy, deterministic_keys, wire};

const OPERATION_MS: u64 = 30_000;
const CUSTOM_KIND: u16 = 50_029;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SimpleGroupFlowFacts {
    pub(crate) simple_group_id: String,
    pub(crate) relay_urls: [String; 2],
    pub(crate) shared_event_id: String,
    pub(crate) unique_event_ids: [String; 2],
    pub(crate) shared_evidence: Vec<String>,
    pub(crate) metadata_names: [String; 2],
    pub(crate) metadata_authors: [String; 2],
    pub(crate) admin_targets: [String; 2],
    pub(crate) admin_authors: [String; 2],
    pub(crate) multi_group_ids: [String; 2],
    pub(crate) multi_group_create_event_ids: [String; 2],
    pub(crate) custom_event_id: String,
    pub(crate) custom_event_signature: String,
    pub(crate) write_id: u64,
    pub(crate) receipt_id: u64,
    pub(crate) custom_destinations: usize,
    pub(crate) custom_acknowledged: usize,
    pub(crate) handoffs: [usize; 2],
    pub(crate) prepared_contexts: usize,
    pub(crate) observation_closed: bool,
}

#[derive(Clone, Debug)]
#[cfg(test)]
pub(crate) struct MultiGroupLiveFacts {
    pub(crate) group_ids: [String; 2],
    pub(crate) event_id: String,
    pub(crate) event_signature: String,
    pub(crate) observed_relays: [String; 2],
}

#[cfg(test)]
pub(crate) async fn execute_multi_group_live_flow(
    root: &Path,
    seed: &str,
    ready: [CroissantReadyFact; 2],
) -> CanaryResult<MultiGroupLiveFacts> {
    fs::create_dir_all(root.join("children"))?;
    let author = deterministic_keys(&format!("multi-group-live-author\0{seed}"))?;
    let relays = [
        RelayUrl::parse(&format!("ws://{}", ready[0].endpoint)).map_err(error)?,
        RelayUrl::parse(&format!("ws://{}", ready[1].endpoint)).map_err(error)?,
    ];
    let group_ids = [
        hex::encode(Sha256::digest(format!("multi-group-a\0{seed}")))[..32].to_owned(),
        hex::encode(Sha256::digest(format!("multi-group-b\0{seed}")))[..32].to_owned(),
    ];
    let groups = [
        SimpleGroup::new(&group_ids[0], vec![relays[0].clone()]).map_err(error)?,
        SimpleGroup::new(&group_ids[1], vec![relays[1].clone()]).map_err(error)?,
    ];
    let publisher = assembly(
        root.join("children/multi-group-publisher.redb"),
        Arc::new(LocalSigner::new(author.clone())),
    )?;
    let observer = assembly(
        root.join("children/multi-group-observer.redb"),
        Arc::new(LocalSigner::new(author.clone())),
    )?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    for (index, group_id) in group_ids.iter().enumerate() {
        let create = EventBuilder::new(author.public_key(), Kind::from_u16(9_007))
            .created_at(Timestamp::from(now + u64::try_from(index).map_err(error)?))
            .content("controlled multi-group live bootstrap")
            .tag(tag(&["h", group_id])?)
            .build()
            .map_err(error)?
            .finalize(&author)
            .map_err(error)?;
        for relay in &relays {
            let receipt = publish_signed(&publisher, [relay.clone()], create.clone()).await?;
            require_terminal(&receipt, 1)?;
        }
    }

    let write = publisher
        .publish(
            EventBuilder::new(author.public_key(), Kind::from_u16(CUSTOM_KIND))
                .created_at(Timestamp::from(now + 2))
                .content("one signed event through two live groups")
                .simple_group(&groups[0])
                .map_err(error)?
                .simple_group(&groups[1])
                .map_err(error)?,
        )
        .map_err(error)?;
    let receipt = wait_terminal(&write).await?;
    require_terminal(&receipt, 2)?;
    let (event_id, event_signature) = match &receipt.current.event {
        fava::EventValue::Signed(event) => (event.id.to_hex(), event.sig.to_string()),
        fava::EventValue::Unsigned(_) => {
            return Err(CanaryError::new("multi-group live event remained unsigned"));
        }
    };

    let mut observed_relays = [String::new(), String::new()];
    for (index, group) in groups.iter().enumerate() {
        let mut observation = observer
            .observe(
                group
                    .events(
                        Query::events()
                            .kinds([Kind::from_u16(CUSTOM_KIND)])
                            .map_err(error)?
                            .limit(1)
                            .map_err(error)?,
                    )
                    .map_err(error)?,
            )
            .await
            .map_err(error)?;
        let expected_id = event_id.clone();
        let expected_signature = event_signature.clone();
        let snapshot = wait_observation(&mut observation, |snapshot| {
            snapshot.events.len() == 1
                && snapshot.events[0].id().to_hex() == expected_id
                && matches!(
                    snapshot.events[0].event(),
                    fava::EventValue::Signed(event)
                        if event.sig.to_string() == expected_signature
                )
                && seen_on(&snapshot.events[0], &relays[index])
        })
        .await?;
        observed_relays[index] = snapshot.events[0]
            .relay_occurrences()
            .occurrences()
            .find(|occurrence| occurrence.session.relay == relays[index])
            .ok_or_else(|| CanaryError::new("multi-group live query lacked exact relay evidence"))?
            .session
            .relay
            .to_string();
        observation.close();
    }

    Ok(MultiGroupLiveFacts {
        group_ids,
        event_id,
        event_signature,
        observed_relays,
    })
}

pub(crate) async fn execute_public_flow(
    root: &Path,
    seed: &str,
    ready: [CroissantReadyFact; 2],
) -> CanaryResult<SimpleGroupFlowFacts> {
    fs::create_dir_all(root.join("wire"))?;
    fs::create_dir_all(root.join("children"))?;
    let proxy_a = WireProxy::start(ready[0].endpoint, &root.join("wire/a.jsonl")).await?;
    let proxy_b = WireProxy::start(ready[1].endpoint, &root.join("wire/b.jsonl")).await?;
    let urls = [proxy_a.url(), proxy_b.url()];
    let result = tokio::time::timeout(
        Duration::from_millis(OPERATION_MS),
        execute_with_proxies(root, seed, &urls),
    )
    .await
    .map_err(|_| CanaryError::new("simple-groups public-flow deadline elapsed"));
    let (stop_a, stop_b) = tokio::join!(proxy_a.shutdown(), proxy_b.shutdown());
    stop_a?;
    stop_b?;
    let (facts, bootstrap_event_id) = result??;
    verify_query_completion(
        &[root.join("wire/a.jsonl"), root.join("wire/b.jsonl")],
        &facts.simple_group_id,
        &facts.multi_group_ids,
        &bootstrap_event_id,
    )?;
    Ok(facts)
}

#[allow(
    clippy::too_many_lines,
    reason = "one linear public flow keeps exact process and wire causality auditable"
)]
async fn execute_with_proxies(
    root: &Path,
    seed: &str,
    urls: &[String; 2],
) -> CanaryResult<(SimpleGroupFlowFacts, String)> {
    let author = deterministic_keys(&format!("simple-groups-author\0{seed}"))?;
    let target_a = deterministic_keys(&format!("simple-groups-admin-a\0{seed}"))?.public_key();
    let target_b = deterministic_keys(&format!("simple-groups-admin-b\0{seed}"))?.public_key();
    let relays = [
        RelayUrl::parse(&urls[0]).map_err(error)?,
        RelayUrl::parse(&urls[1]).map_err(error)?,
    ];
    let simple_group_id =
        hex::encode(Sha256::digest(format!("simple-groups\0{seed}")))[..32].to_owned();

    let simple_group =
        SimpleGroup::new(&simple_group_id, vec![relays[0].clone(), relays[1].clone()])
            .map_err(error)?;
    let signer: Arc<dyn Signer> = Arc::new(LocalSigner::new(author.clone()));
    let publisher = assembly(root.join("children/publisher.redb"), signer)?;
    let observer = assembly(
        root.join("children/observer.redb"),
        Arc::new(LocalSigner::new(author.clone())),
    )?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let create = EventBuilder::new(author.public_key(), Kind::from_u16(9_007))
        .created_at(Timestamp::from(now))
        .content("controlled group bootstrap")
        .tag(tag(&["h", &simple_group_id])?)
        .build()
        .map_err(error)?
        .finalize(&author)
        .map_err(error)?;
    for (index, relay) in relays.iter().enumerate() {
        let receipt = publish_signed(&publisher, [relay.clone()], create.clone()).await?;
        require_terminal(&receipt, 1)?;
        let witness =
            wire::query_exact(&urls[index], create.id, &format!("group-create-{index}")).await?;
        if !witness.found_event || !witness.saw_eose {
            return Err(CanaryError::new(
                "group bootstrap was not independently readable",
            ));
        }
    }

    for (index, (relay, name, about)) in [
        (&relays[0], "relay-A", "A-only metadata"),
        (&relays[1], "relay-B", "B-only metadata"),
    ]
    .into_iter()
    .enumerate()
    {
        let metadata = EventBuilder::new(author.public_key(), Kind::from_u16(9_002))
            .created_at(Timestamp::from(now + 1 + index as u64))
            .tags([
                tag(&["name", name])?,
                tag(&["about", about])?,
                tag(&["h", &simple_group_id])?,
            ])
            .build()
            .map_err(error)?;
        require_terminal(
            &publish_unsigned(&publisher, [relay.clone()], metadata).await?,
            1,
        )?;
    }
    for (index, (relay, target)) in [(&relays[0], target_a), (&relays[1], target_b)]
        .into_iter()
        .enumerate()
    {
        let admin = EventBuilder::new(author.public_key(), Kind::from_u16(9_000))
            .created_at(Timestamp::from(now + 3 + index as u64))
            .tags([
                tag(&["p", &target.to_hex(), "admin"])?,
                tag(&["h", &simple_group_id])?,
            ])
            .build()
            .map_err(error)?;
        require_terminal(
            &publish_unsigned(&publisher, [relay.clone()], admin).await?,
            1,
        )?;
    }

    let shared = signed_group_event(&simple_group, &author, now + 5, "shared-content")?;
    let unique_a = signed_group_event(&simple_group, &author, now + 6, "unique-A")?;
    let unique_b = signed_group_event(&simple_group, &author, now + 7, "unique-B")?;
    require_terminal(
        &publish_signed(&publisher, relays.clone(), shared.clone()).await?,
        2,
    )?;
    require_terminal(
        &publish_signed(&publisher, [relays[0].clone()], unique_a.clone()).await?,
        1,
    )?;
    require_terminal(
        &publish_signed(&publisher, [relays[1].clone()], unique_b.clone()).await?,
        1,
    )?;

    let mut content = observer
        .observe(
            simple_group
                .events(
                    Query::events()
                        .kinds([Kind::from_u16(9)])
                        .map_err(error)?
                        .limit(16)
                        .map_err(error)?,
                )
                .map_err(error)?,
        )
        .await
        .map_err(error)?;
    let mut records = observer
        .observe(
            simple_group
                .meta_events(SimpleGroupStateEventKind::ALL)
                .map_err(error)?,
        )
        .await
        .map_err(error)?;
    let selected_relays = simple_group.relays().count();
    let content_snapshot = wait_observation(&mut content, |current| {
        current.events.len() == selected_relays + 1
            && current.events.iter().any(|record| {
                record.id() == shared.id && record.relay_occurrences().len() == selected_relays
            })
    })
    .await?;
    let record_snapshot = wait_observation(&mut records, |current| {
        relays.iter().all(|relay| {
            metadata_for_relay(current, relay).is_some()
                && admins_for_relay(current, relay, author.public_key()).is_some()
        })
    })
    .await?;
    let metadata = [
        metadata_for_relay(&record_snapshot, &relays[0])
            .ok_or_else(|| CanaryError::new("relay A metadata was absent"))?,
        metadata_for_relay(&record_snapshot, &relays[1])
            .ok_or_else(|| CanaryError::new("relay B metadata was absent"))?,
    ];
    let admins = [
        admins_for_relay(&record_snapshot, &relays[0], author.public_key())
            .ok_or_else(|| CanaryError::new("relay A admin state was absent"))?,
        admins_for_relay(&record_snapshot, &relays[1], author.public_key())
            .ok_or_else(|| CanaryError::new("relay B admin state was absent"))?,
    ];
    if metadata[0].0 == metadata[1].0 || admins[0].0 == admins[1].0 {
        return Err(CanaryError::new(
            "relay-local metadata/admin records did not remain distinct",
        ));
    }

    let observed_shared_evidence = content_snapshot
        .events
        .iter()
        .find(|record| record.id() == shared.id)
        .ok_or_else(|| CanaryError::new("shared content was absent"))?
        .relay_occurrences()
        .occurrences()
        .map(|occurrence| occurrence.session.relay.to_string())
        .collect::<Vec<_>>();
    let shared_evidence = relays.iter().map(ToString::to_string).collect::<Vec<_>>();
    if observed_shared_evidence.len() != shared_evidence.len()
        || shared_evidence
            .iter()
            .any(|relay| !observed_shared_evidence.contains(relay))
    {
        return Err(CanaryError::new(
            "shared content evidence did not match the exact relay route",
        ));
    }
    for (relay, expected) in [(&relays[0], "relay-A"), (&relays[1], "relay-B")] {
        if metadata_for_relay(&record_snapshot, relay).map(|value| value.0)
            != Some(expected.to_owned())
        {
            return Err(CanaryError::new(
                "relay-evidence selection returned another relay's metadata",
            ));
        }
    }

    let multi_group_ids = [
        format!("{simple_group_id}-custom-a"),
        format!("{simple_group_id}-custom-b"),
    ];
    let custom_groups = [
        SimpleGroup::new(&multi_group_ids[0], vec![relays[0].clone()]).map_err(error)?,
        SimpleGroup::new(&multi_group_ids[1], vec![relays[1].clone()]).map_err(error)?,
    ];
    let mut multi_group_create_event_ids = [String::new(), String::new()];
    for index in 0..custom_groups.len() {
        let create = EventBuilder::new(author.public_key(), Kind::from_u16(9_007))
            .created_at(Timestamp::from(now + 8))
            .content("controlled multi-group bootstrap")
            .tag(tag(&["h", &multi_group_ids[index]])?)
            .build()
            .map_err(error)?
            .finalize(&author)
            .map_err(error)?;
        for relay in &relays {
            let receipt = publish_signed(&publisher, [relay.clone()], create.clone()).await?;
            require_terminal(&receipt, 1)?;
        }
        multi_group_create_event_ids[index] = create.id.to_hex();
    }
    let custom = EventBuilder::new(author.public_key(), Kind::from_u16(CUSTOM_KIND))
        .created_at(Timestamp::from(now + 9))
        .content("one arbitrary event across two exact groups")
        .simple_group(&custom_groups[0])
        .map_err(error)?
        .simple_group(&custom_groups[1])
        .map_err(error)?;
    let custom_write = publisher.publish(custom).map_err(error)?;
    let custom_receipt = wait_terminal(&custom_write).await?;
    require_terminal(&custom_receipt, selected_relays)?;
    let custom_id = custom_receipt.current.id();
    let custom_event_signature = match &custom_receipt.current.event {
        fava::EventValue::Signed(event) => event.sig.to_string(),
        fava::EventValue::Unsigned(_) => {
            return Err(CanaryError::new(
                "multi-group publication retained an unsigned terminal event",
            ));
        }
    };
    let handoffs = [
        exact_event_handoffs(&root.join("wire/a.jsonl"), custom_id)?,
        exact_event_handoffs(&root.join("wire/b.jsonl"), custom_id)?,
    ];
    for group in &custom_groups {
        let mut observation = observer
            .observe(
                group
                    .events(
                        Query::events()
                            .kinds([Kind::from_u16(CUSTOM_KIND)])
                            .map_err(error)?
                            .limit(1)
                            .map_err(error)?,
                    )
                    .map_err(error)?,
            )
            .await
            .map_err(error)?;
        wait_observation(&mut observation, |snapshot| {
            snapshot.events.len() == 1 && snapshot.events[0].id() == custom_id
        })
        .await?;
        observation.close();
    }

    let before_preparation = publisher.open_receipts().map_err(error)?.len();
    let drafts = [
        EventBuilder::new(author.public_key(), Kind::from_u16(CUSTOM_KIND + 1))
            .created_at(Timestamp::from(now + 9))
            .content("missing context"),
        EventBuilder::new(author.public_key(), Kind::from_u16(CUSTOM_KIND + 1))
            .created_at(Timestamp::from(now + 10))
            .tags([
                tag(&["h", &simple_group_id])?,
                tag(&["h", &simple_group_id, "unused"])?,
            ])
            .content("extended context"),
        EventBuilder::new(author.public_key(), Kind::from_u16(CUSTOM_KIND + 1))
            .created_at(Timestamp::from(now + 11))
            .tag(tag(&["h", "other-group"])?)
            .content("foreign context"),
    ];
    let mut prepared_contexts = 0;
    for draft in drafts {
        let (event, _) = draft
            .simple_group(&simple_group)
            .map_err(error)?
            .into_event_and_routing()
            .map_err(error)?;
        if event
            .tags
            .iter()
            .any(|tag| tag.as_slice() == ["h", simple_group_id.as_str()])
        {
            prepared_contexts += 1;
        }
    }
    if publisher.open_receipts().map_err(error)?.len() != before_preparation {
        return Err(CanaryError::new(
            "pure simple-group builder composition reached Fava custody",
        ));
    }

    content.close();
    content.close();
    records.close();
    records.close();
    let observation_closed = content.changed().await.is_err() && records.changed().await.is_err();
    wait_for_query_completion(
        &[root.join("wire/a.jsonl"), root.join("wire/b.jsonl")],
        &simple_group_id,
        &multi_group_ids,
        &create.id.to_hex(),
    )
    .await?;

    let bootstrap_event_id = create.id.to_hex();
    Ok((
        SimpleGroupFlowFacts {
            simple_group_id,
            relay_urls: [relays[0].to_string(), relays[1].to_string()],
            shared_event_id: shared.id.to_hex(),
            unique_event_ids: [unique_a.id.to_hex(), unique_b.id.to_hex()],
            shared_evidence,
            metadata_names: [metadata[0].0.clone(), metadata[1].0.clone()],
            metadata_authors: [metadata[0].1.clone(), metadata[1].1.clone()],
            admin_targets: [admins[0].0.clone(), admins[1].0.clone()],
            admin_authors: [admins[0].1.clone(), admins[1].1.clone()],
            multi_group_ids,
            multi_group_create_event_ids,
            custom_event_id: custom_id.to_hex(),
            custom_event_signature,
            write_id: custom_receipt.write_id.as_u64(),
            receipt_id: custom_receipt.receipt_id.as_u64(),
            custom_destinations: custom_receipt.desired_destinations.len(),
            custom_acknowledged: custom_receipt.acknowledged(),
            handoffs,
            prepared_contexts,
            observation_closed,
        },
        bootstrap_event_id,
    ))
}

fn metadata_for_relay(
    snapshot: &fava::QuerySnapshot,
    relay: &RelayUrl,
) -> Option<(String, String)> {
    snapshot.events.iter().find_map(|record| {
        if !seen_on(record, relay) {
            return None;
        }
        let metadata = SimpleGroupMetadata::from_event(record.event()).ok()?;
        Some((
            metadata.name().unwrap_or_default().to_owned(),
            metadata.author().to_hex(),
        ))
    })
}

fn admins_for_relay(
    snapshot: &fava::QuerySnapshot,
    relay: &RelayUrl,
    author: fava::PublicKey,
) -> Option<(String, String)> {
    snapshot.events.iter().find_map(|record| {
        if !seen_on(record, relay) {
            return None;
        }
        let admins = SimpleGroupAdmins::from_event(record.event()).ok()?;
        let target = admins
            .admins()
            .iter()
            .filter_map(|entry| entry.as_ref().ok())
            .map(|(key, _)| key.clone())
            .find(|key| key != &author.to_hex())?;
        Some((target, admins.author().to_hex()))
    })
}

fn seen_on(record: &fava::EventRecord, relay: &RelayUrl) -> bool {
    record
        .relay_occurrences()
        .occurrences()
        .any(|occurrence| &occurrence.session.relay == relay)
}

fn assembly(database: PathBuf, signer: Arc<dyn Signer>) -> CanaryResult<Fava> {
    Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(RedbWriteStore::open(database).map_err(error)?))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .signers([signer])
        .build()
        .map_err(error)
}

async fn publish_unsigned(
    fava: &Fava,
    relays: impl IntoIterator<Item = RelayUrl>,
    event: fava::UnsignedEvent,
) -> CanaryResult<Receipt> {
    let write = fava
        .to(relays)
        .map_err(error)?
        .publish(event)
        .map_err(error)?;
    wait_terminal(&write).await
}

async fn publish_signed(
    fava: &Fava,
    relays: impl IntoIterator<Item = RelayUrl>,
    event: Event,
) -> CanaryResult<Receipt> {
    let write = fava
        .to(relays)
        .map_err(error)?
        .publish(event)
        .map_err(error)?;
    wait_terminal(&write).await
}

fn require_terminal(receipt: &Receipt, destinations: usize) -> CanaryResult<()> {
    if receipt.outcome != ReceiptOutcome::Complete
        || receipt.desired_destinations.len() != destinations
        || receipt.acknowledged() != destinations
        || !receipt.attempts.values().all(|attempts| *attempts == 1)
    {
        return Err(CanaryError::new(format!(
            "exact publication receipt was incomplete: outcome={:?}, desired={}, acknowledged={}, attempts={:?}",
            receipt.outcome,
            receipt.desired_destinations.len(),
            receipt.acknowledged(),
            receipt.attempts
        )));
    }
    Ok(())
}

fn signed_group_event(
    simple_group: &SimpleGroup,
    keys: &Keys,
    created: u64,
    content: &str,
) -> CanaryResult<Event> {
    EventBuilder::new(keys.public_key(), Kind::from_u16(9))
        .created_at(Timestamp::from(created))
        .content(content)
        .tag(tag(&["h", simple_group.id()])?)
        .build()
        .map_err(error)?
        .finalize(keys)
        .map_err(error)
}

async fn wait_observation(
    observation: &mut Observation,
    predicate: impl Fn(&fava::QuerySnapshot) -> bool,
) -> CanaryResult<Arc<fava::QuerySnapshot>> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let current = observation.current();
            if predicate(&current) {
                return Ok(current);
            }
            observation.changed().await.map_err(error)?;
        }
    })
    .await
    .map_err(|_| CanaryError::new("simple-groups observation deadline elapsed"))?
}

fn tag(values: &[&str]) -> CanaryResult<Tag> {
    Tag::parse(values.iter().copied()).map_err(error)
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
