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
use fava_simple_groups::{Group, GroupRecords};
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write::{Event, ReceiptOutcome};
use fava_write_store_redb::RedbWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
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
pub(crate) struct SimpleGroupsFlowFacts {
    pub(crate) group_id: String,
    pub(crate) relay_urls: [String; 2],
    pub(crate) shared_event_id: String,
    pub(crate) unique_event_ids: [String; 2],
    pub(crate) shared_evidence: Vec<String>,
    pub(crate) metadata_names: [String; 2],
    pub(crate) metadata_authors: [String; 2],
    pub(crate) admin_targets: [String; 2],
    pub(crate) admin_authors: [String; 2],
    pub(crate) custom_event_id: String,
    pub(crate) write_id: u64,
    pub(crate) receipt_id: u64,
    pub(crate) custom_destinations: usize,
    pub(crate) custom_acknowledged: usize,
    pub(crate) handoffs: [usize; 2],
    pub(crate) signed_refusals: usize,
    pub(crate) observation_closed: bool,
}

pub(crate) async fn execute_public_flow(
    root: &Path,
    seed: &str,
    ready: [CroissantReadyFact; 2],
) -> CanaryResult<SimpleGroupsFlowFacts> {
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
        &facts.group_id,
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
) -> CanaryResult<(SimpleGroupsFlowFacts, String)> {
    let author = deterministic_keys(&format!("simple-groups-author\0{seed}"))?;
    let target_a = deterministic_keys(&format!("simple-groups-admin-a\0{seed}"))?.public_key();
    let target_b = deterministic_keys(&format!("simple-groups-admin-b\0{seed}"))?.public_key();
    let relays = [
        RelayUrl::parse(&urls[0]).map_err(error)?,
        RelayUrl::parse(&urls[1]).map_err(error)?,
    ];
    let group_id = hex::encode(Sha256::digest(format!("simple-groups\0{seed}")))[..32].to_owned();

    let group = Group::on(relays.clone(), &group_id).map_err(error)?;
    let signer: Arc<dyn Signer> = Arc::new(LocalSigner::new(author.clone()));
    let publisher = assembly(root.join("children/publisher.redb"), signer)?;
    let observer = assembly(
        root.join("children/observer.redb"),
        Arc::new(LocalSigner::new(author.clone())),
    )?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let create = group
        .prepare(
            EventBuilder::new(author.public_key(), Kind::from_u16(9_007))
                .created_at(Timestamp::from(now))
                .content("controlled group bootstrap")
                .build()
                .map_err(error)?,
        )
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
        let metadata = group
            .edit_metadata(
                EventBuilder::new(author.public_key(), Kind::from_u16(9_002))
                    .created_at(Timestamp::from(now + 1 + index as u64))
                    .tags([tag(&["name", name])?, tag(&["about", about])?])
                    .build()
                    .map_err(error)?,
            )
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
        let admin = group
            .prepare(
                EventBuilder::new(author.public_key(), Kind::from_u16(9_000))
                    .created_at(Timestamp::from(now + 3 + index as u64))
                    .tag(tag(&["p", &target.to_hex(), "admin"])?)
                    .build()
                    .map_err(error)?,
            )
            .map_err(error)?;
        require_terminal(
            &publish_unsigned(&publisher, [relay.clone()], admin).await?,
            1,
        )?;
    }

    let shared = signed_group_event(&group, &author, now + 5, "shared-content")?;
    let unique_a = signed_group_event(&group, &author, now + 6, "unique-A")?;
    let unique_b = signed_group_event(&group, &author, now + 7, "unique-B")?;
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
            group
                .events(
                    Query::events()
                        .kind(Kind::from_u16(9))
                        .limit(16)
                        .map_err(error)?,
                )
                .map_err(error)?,
        )
        .await
        .map_err(error)?;
    let mut records = observer
        .observe(group.records(GroupRecords::all()).map_err(error)?)
        .await
        .map_err(error)?;
    let selected_hosts = group.hosts().count();
    let content_snapshot = wait_observation(&mut content, |current| {
        current.events.len() == selected_hosts + 1
            && current.events.iter().any(|record| {
                record.id() == shared.id && record.relay_evidence.len() == selected_hosts
            })
    })
    .await?;
    let record_snapshot = wait_observation(&mut records, |current| {
        let Ok(projected) = group.project(current) else {
            return false;
        };
        projected.metadata().count() == selected_hosts
            && projected.admin_records().count() == selected_hosts
    })
    .await?;
    let projected = group.project(&record_snapshot).map_err(error)?;
    if selected_hosts == 2 && (!projected.metadata_differ() || !projected.admins_differ()) {
        return Err(CanaryError::new(
            "host-local metadata/admin forks did not disagree",
        ));
    }
    let metadata = projected
        .metadata()
        .map(|(_, value)| {
            (
                value.name().unwrap_or_default().to_owned(),
                value.author().to_hex(),
            )
        })
        .collect::<Vec<_>>();
    let admins = projected
        .admin_records()
        .map(|(_, value)| {
            let target = value
                .admins()
                .iter()
                .filter_map(|row| row.as_ref().ok())
                .map(|(key, _)| key.to_hex())
                .find(|key| key != &author.public_key().to_hex())
                .unwrap_or_default();
            (target, value.author().to_hex())
        })
        .collect::<Vec<_>>();
    let metadata = [
        metadata.first().cloned().unwrap_or_default(),
        metadata.get(1).cloned().unwrap_or_default(),
    ];
    let admins = [
        admins.first().cloned().unwrap_or_default(),
        admins.get(1).cloned().unwrap_or_default(),
    ];

    let observed_shared_evidence = content_snapshot
        .events
        .iter()
        .find(|record| record.id() == shared.id)
        .ok_or_else(|| CanaryError::new("shared content was absent"))?
        .relay_evidence
        .observations()
        .map(|observation| observation.session.relay.to_string())
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
    for (relay, expected) in [(&relays[0], "relay-A"), (&relays[1], "relay-B")]
        .into_iter()
        .take(selected_hosts)
    {
        let selected = Group::on([relay.clone()], &group_id)
            .map_err(error)?
            .project(&record_snapshot)
            .map_err(error)?;
        if selected
            .metadata()
            .next()
            .and_then(|(_, value)| value.name())
            != Some(expected)
        {
            return Err(CanaryError::new(
                "single-host fork choice selected another host",
            ));
        }
    }

    let custom = group
        .prepare(
            EventBuilder::new(author.public_key(), Kind::from_u16(CUSTOM_KIND))
                .created_at(Timestamp::from(now + 8))
                .content("arbitrary kind across exact hosts")
                .build()
                .map_err(error)?,
        )
        .map_err(error)?;
    let custom_write = publisher
        .to(group.hosts())
        .map_err(error)?
        .publish(custom)
        .map_err(error)?;
    let custom_receipt = wait_terminal(&custom_write).await?;
    require_terminal(&custom_receipt, selected_hosts)?;
    let custom_id = custom_receipt.current.id();
    let handoffs = [
        exact_event_handoffs(&root.join("wire/a.jsonl"), custom_id)?,
        exact_event_handoffs(&root.join("wire/b.jsonl"), custom_id)?,
    ];

    let before_refusal = publisher.open_receipts().map_err(error)?.len();
    let invalids = [
        signed_raw(&author, now + 9, vec![])?,
        signed_raw(
            &author,
            now + 10,
            vec![tag(&["h", &group_id])?, tag(&["h", &group_id])?],
        )?,
        signed_raw(&author, now + 11, vec![tag(&["h", "other-group"])?])?,
    ];
    let mut signed_refusals = 0;
    for invalid in invalids {
        if group.prepare(invalid).is_err() {
            signed_refusals += 1;
        }
    }
    if publisher.open_receipts().map_err(error)?.len() != before_refusal {
        return Err(CanaryError::new(
            "invalid signed context reached Fava custody",
        ));
    }

    content.close();
    content.close();
    records.close();
    records.close();
    let observation_closed = content.changed().await.is_err() && records.changed().await.is_err();
    wait_for_query_completion(
        &[root.join("wire/a.jsonl"), root.join("wire/b.jsonl")],
        &group_id,
        &create.id.to_hex(),
    )
    .await?;

    let bootstrap_event_id = create.id.to_hex();
    Ok((
        SimpleGroupsFlowFacts {
            group_id,
            relay_urls: [relays[0].to_string(), relays[1].to_string()],
            shared_event_id: shared.id.to_hex(),
            unique_event_ids: [unique_a.id.to_hex(), unique_b.id.to_hex()],
            shared_evidence,
            metadata_names: [metadata[0].0.clone(), metadata[1].0.clone()],
            metadata_authors: [metadata[0].1.clone(), metadata[1].1.clone()],
            admin_targets: [admins[0].0.clone(), admins[1].0.clone()],
            admin_authors: [admins[0].1.clone(), admins[1].1.clone()],
            custom_event_id: custom_id.to_hex(),
            write_id: custom_receipt.write_id.as_u64(),
            receipt_id: custom_receipt.receipt_id.as_u64(),
            custom_destinations: custom_receipt.desired_destinations.len(),
            custom_acknowledged: custom_receipt.acknowledged(),
            handoffs,
            signed_refusals,
            observation_closed,
        },
        bootstrap_event_id,
    ))
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
    group: &Group,
    keys: &Keys,
    created: u64,
    content: &str,
) -> CanaryResult<Event> {
    group
        .prepare(
            EventBuilder::new(keys.public_key(), Kind::from_u16(9))
                .created_at(Timestamp::from(created))
                .content(content)
                .build()
                .map_err(error)?,
        )
        .map_err(error)?
        .finalize(keys)
        .map_err(error)
}

fn signed_raw(keys: &Keys, created: u64, tags: Vec<Tag>) -> CanaryResult<Event> {
    NostrEventBuilder::new(Kind::from_u16(CUSTOM_KIND + 1), "invalid signed context")
        .tags(tags)
        .custom_created_at(Timestamp::from(created))
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
