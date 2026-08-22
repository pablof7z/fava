//! Public-facade M1 scenarios over independently assembled memory providers.

use std::sync::Arc;
use std::time::Duration;

use fava::{EventValue, Fava, Query};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder, FinalizeEvent, FinalizeUnsignedEvent, Kind, Tag};
use nostr::key::Keys;

use crate::{CanaryError, CanaryResult};

/// Run one M1 local scenario through the public Fava facade.
///
/// # Errors
///
/// Returns an error when assembly, event state, query observation, or a
/// scenario assertion fails.
pub async fn run_local_scenario(id: &str, seed: &str) -> CanaryResult<usize> {
    let keys = super::deterministic_keys(&format!("{id}\0{seed}"))?;
    let cache = Arc::new(MemoryEventCache::default());
    let writes = Arc::new(MemoryWriteStore::default());
    let fava = Fava::builder()
        .event_cache(Arc::clone(&cache))
        .write_store(Arc::clone(&writes))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .map_err(error)?;
    let count = match id {
        "local-source-merge" => source_merge(&fava, &cache, &writes, &keys).await?,
        "local-replaceable-shadow-and-cancel" => {
            replaceable_shadow(&fava, &cache, &writes, &keys).await?
        }
        "local-source-removal" => source_removal(&fava, &cache, &keys).await?,
        "slow-consumer-latest-state" => slow_consumer(&fava, &writes, &keys).await?,
        _ => return Err(CanaryError::new(format!("unknown local scenario: {id}"))),
    };
    Ok(count)
}

async fn slow_consumer(fava: &Fava, writes: &MemoryWriteStore, keys: &Keys) -> CanaryResult<usize> {
    let mut observation = fava
        .observe(Query::events().cache_only())
        .await
        .map_err(error)?;
    for _ in 0..128 {
        if tokio::time::timeout(Duration::ZERO, observation.changed())
            .await
            .is_ok()
        {
            return Err(CanaryError::new(
                "idle cancelled pull unexpectedly delivered state",
            ));
        }
    }
    for index in 0..256 {
        let event = EventBuilder::new(Kind::TextNote, format!("burst-{index}"))
            .custom_created_at(Timestamp::from(index + 1))
            .finalize(keys)
            .map_err(error)?;
        writes
            .accept_materialized(EventValue::Signed(event))
            .map_err(error)?;
    }
    tokio::time::timeout(Duration::from_secs(2), async {
        while observation.current().events.len() != 256 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .map_err(|_| CanaryError::new("latest-state production deadline elapsed"))?;
    let current = observation.changed().await.map_err(error)?;
    require(
        current.events.len() == 256,
        "slow observer did not receive one exact latest state",
    )?;
    require(
        fava.diagnostics().coalesced_query_updates > 0,
        "coalesced current-state updates were not measured",
    )?;
    Ok(current.events.len())
}

async fn source_merge(
    fava: &Fava,
    cache: &MemoryEventCache,
    writes: &MemoryWriteStore,
    keys: &Keys,
) -> CanaryResult<usize> {
    let event = EventBuilder::new(Kind::TextNote, "same event")
        .custom_created_at(Timestamp::from(10))
        .finalize(keys)
        .map_err(error)?;
    let accepted = writes
        .accept_materialized(EventValue::Signed(event.clone()))
        .map_err(error)?;
    cache
        .admit(cached(event), Timestamp::from(11))
        .map_err(error)?;
    let observation = fava
        .observe(Query::events().cache_only())
        .await
        .map_err(error)?;
    let events = &observation.current().events;
    require(events.len() == 1, "same event was not deduplicated")?;
    require(
        events[0].relay_evidence.len() == 1,
        "relay evidence was not retained",
    )?;
    require(
        events[0]
            .publication
            .as_ref()
            .is_some_and(|value| value.receipt_id == accepted.receipt_id),
        "publication evidence was not retained",
    )?;
    Ok(events.len())
}

async fn replaceable_shadow(
    fava: &Fava,
    cache: &MemoryEventCache,
    writes: &MemoryWriteStore,
    keys: &Keys,
) -> CanaryResult<usize> {
    let predecessor = EventBuilder::new(Kind::ContactList, "cached")
        .custom_created_at(Timestamp::from(10))
        .finalize(keys)
        .map_err(error)?;
    cache
        .admit(cached(predecessor.clone()), Timestamp::from(11))
        .map_err(error)?;
    let mut successor = EventBuilder::new(Kind::ContactList, "local")
        .custom_created_at(Timestamp::from(20))
        .finalize_unsigned(keys.public_key());
    successor.ensure_id();
    let successor_id = successor
        .id
        .ok_or_else(|| CanaryError::new("missing event id"))?;
    let accepted = writes
        .accept_materialized(EventValue::Unsigned(successor))
        .map_err(error)?;
    let mut observation = fava
        .observe(Query::events().kind(Kind::ContactList).cache_only())
        .await
        .map_err(error)?;
    require(
        observation
            .current()
            .events
            .first()
            .map(fava::EventRecord::id)
            == Some(successor_id),
        "local successor did not shadow cached predecessor",
    )?;
    fava.cancel_write(accepted.receipt_id).map_err(error)?;
    observation.changed().await.map_err(error)?;
    let events = &observation.current().events;
    require(
        events.first().map(fava::EventRecord::id) == Some(predecessor.id),
        "cancellation did not reveal cached predecessor",
    )?;
    Ok(events.len())
}

async fn source_removal(fava: &Fava, cache: &MemoryEventCache, keys: &Keys) -> CanaryResult<usize> {
    let event = EventBuilder::new(Kind::TextNote, "temporary")
        .tag(Tag::expiration(Timestamp::from(20)))
        .custom_created_at(Timestamp::from(10))
        .finalize(keys)
        .map_err(error)?;
    cache
        .admit(cached(event), Timestamp::from(11))
        .map_err(error)?;
    let mut observation = fava
        .observe(Query::events().kind(Kind::TextNote).cache_only())
        .await
        .map_err(error)?;
    require(
        observation.current().events.len() == 1,
        "event was not visible",
    )?;
    cache.expire(Timestamp::from(20)).map_err(error)?;
    observation.changed().await.map_err(error)?;
    require(
        observation.current().events.is_empty(),
        "expired event remained visible",
    )?;
    Ok(0)
}

fn cached(event: nostr::event::Event) -> CachedEvent {
    CachedEvent::new(
        event,
        RelayEvidence::one(
            RelaySessionKey::new(
                RelayUrl::parse("wss://m1.local").expect("constant relay URL"),
                RelayAccess::public(),
            ),
            Timestamp::from(11),
        ),
    )
}

fn require(condition: bool, message: &str) -> CanaryResult<()> {
    condition
        .then_some(())
        .ok_or_else(|| CanaryError::new(message))
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
