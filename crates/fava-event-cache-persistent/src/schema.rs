//! Redb table definitions and serialization for durable event state.

use std::collections::BTreeMap;

use fava_relay::Authority;
use fava_state::RelayEvent;
use nostr::event::{Event, EventId};
use nostr::key::PublicKey;
use nostr::types::{RelayUrl, Timestamp};
use redb::{Database, Durability, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

pub(super) const EVENTS: TableDefinition<&str, &[u8]> = TableDefinition::new("events");
/// Redb table holding the on-disk schema version.
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
const SCHEMA_VERSION_KEY: &str = "schema_version";
const SCHEMA_VERSION: u64 = 1;

#[derive(Serialize, Deserialize)]
struct PersistedEvent {
    event_json: serde_json::Value,
    relay_url: String,
    authority: PersistedAuthority,
    observed_at: u64,
}

#[derive(Serialize, Deserialize)]
enum PersistedAuthority {
    Unauthenticated,
    As(String),
}

pub(super) fn composite_key(event_id: EventId, session: &RelayUrl) -> String {
    format!("{}:{}", event_id.to_hex(), session)
}

fn to_bytes(event: &RelayEvent) -> Result<Vec<u8>, String> {
    let event_json = serde_json::to_value(event.event()).map_err(|e| e.to_string())?;
    let persisted = PersistedEvent {
        event_json,
        relay_url: event.occurrence().session.to_string(),
        authority: match &event.occurrence().authority {
            Authority::Unauthenticated => PersistedAuthority::Unauthenticated,
            Authority::As(pk) => PersistedAuthority::As(pk.to_hex()),
        },
        observed_at: event.occurrence().observed_at.as_secs(),
    };
    serde_json::to_vec(&persisted).map_err(|e| e.to_string())
}

fn from_bytes(data: &[u8]) -> Result<RelayEvent, String> {
    let persisted: PersistedEvent = serde_json::from_slice(data).map_err(|e| e.to_string())?;
    let event: Event = serde_json::from_value(persisted.event_json).map_err(|e| e.to_string())?;
    let relay_url = RelayUrl::parse(&persisted.relay_url).map_err(|e| e.to_string())?;
    let authority = match persisted.authority {
        PersistedAuthority::Unauthenticated => Authority::Unauthenticated,
        PersistedAuthority::As(hex) => {
            Authority::As(PublicKey::from_hex(&hex).map_err(|e| e.to_string())?)
        }
    };
    Ok(RelayEvent::new(
        event,
        relay_url,
        authority,
        Timestamp::from_secs(persisted.observed_at),
    ))
}

pub(super) fn initialize(database: &Database, is_new: bool) -> Result<(), String> {
    if !is_new {
        return validate_schema(database);
    }
    let mut txn = database.begin_write().map_err(|e| e.to_string())?;
    txn.set_durability(Durability::Immediate)
        .map_err(|e| e.to_string())?;
    txn.open_table(EVENTS).map_err(|e| e.to_string())?;
    {
        let mut meta = txn.open_table(META).map_err(|e| e.to_string())?;
        meta.insert(SCHEMA_VERSION_KEY, SCHEMA_VERSION)
            .map_err(|e| e.to_string())?;
    }
    txn.commit().map_err(|e| e.to_string())
}

fn validate_schema(database: &Database) -> Result<(), String> {
    let txn = database.begin_read().map_err(|e| e.to_string())?;
    let meta = txn.open_table(META).map_err(|e| e.to_string())?;
    let version = meta
        .get(SCHEMA_VERSION_KEY)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "event-cache schema version missing".to_owned())?
        .value();
    if version != SCHEMA_VERSION {
        return Err(format!(
            "event-cache schema version {version} != {SCHEMA_VERSION}"
        ));
    }
    Ok(())
}

pub(super) fn load(
    database: &Database,
) -> Result<BTreeMap<(EventId, RelayUrl), RelayEvent>, String> {
    let txn = database.begin_read().map_err(|e| e.to_string())?;
    let table = txn.open_table(EVENTS).map_err(|e| e.to_string())?;
    let mut events = BTreeMap::new();
    for entry in table.iter().map_err(|e| e.to_string())? {
        let (_, value) = entry.map_err(|e| e.to_string())?;
        let relay_event = from_bytes(value.value())?;
        let key = (
            relay_event.event().id,
            relay_event.occurrence().session.clone(),
        );
        events.insert(key, relay_event);
    }
    Ok(events)
}

pub(super) fn apply_diff(
    database: &Database,
    inserted: &[RelayEvent],
    removed: &[(EventId, RelayUrl)],
) -> Result<(), String> {
    if inserted.is_empty() && removed.is_empty() {
        return Ok(());
    }
    let mut txn = database.begin_write().map_err(|e| e.to_string())?;
    txn.set_durability(Durability::Immediate)
        .map_err(|e| e.to_string())?;
    {
        let mut table = txn.open_table(EVENTS).map_err(|e| e.to_string())?;
        for relay_event in inserted {
            let key = composite_key(relay_event.event().id, &relay_event.occurrence().session);
            let value = to_bytes(relay_event)?;
            table
                .insert(key.as_str(), value.as_slice())
                .map_err(|e| e.to_string())?;
        }
        for (event_id, session) in removed {
            let key = composite_key(*event_id, session);
            table.remove(key.as_str()).map_err(|e| e.to_string())?;
        }
    }
    txn.commit().map_err(|e| e.to_string())
}
