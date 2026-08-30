//! Phase E exit gate scenarios: persistent event cache and fetch-cache evidence.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_event_cache_persistent::RedbEventCache;
use fava_fetch_cache::{FetchFuture, HttpFetcher, HttpResponse, MemoryFetchCache};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_state::RelayEvent;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

use crate::{CanaryError, CanaryResult};

/// Evidence summary from all four Phase E exit gate scenarios.
pub struct PhaseEOutcome {
    /// Gate 2: ephemeral-cache restart — empty after new instance.
    pub gate2_ephemeral: String,
    /// Gate 3: persistent-cache restart — events survive reopen.
    pub gate3_persistent: String,
    /// Gate 4: NIP-05 negative-cache — zero refetch on second resolve.
    pub gate4_nip05: String,
    /// Gate 5: NIP-11 stale-result — stale evidence after TTL expires.
    pub gate5_nip11: String,
}

/// Run all four Phase E exit gate scenarios in-process.
///
/// Creates evidence artefacts under `runs_directory/phase-e/`.
///
/// # Errors
///
/// Returns [`CanaryError`] when any gate fails to produce its expected evidence.
pub async fn run_phase_e_gates(runs_directory: PathBuf) -> CanaryResult<PhaseEOutcome> {
    let dir = runs_directory.join("phase-e");
    std::fs::create_dir_all(&dir)?;
    let gate2 = gate2_ephemeral_restart()?;
    let gate3 = gate3_persistent_restart(&dir)?;
    let gate4 = gate4_nip05_negative_cache().await?;
    let gate5 = gate5_nip11_stale_result().await?;
    Ok(PhaseEOutcome {
        gate2_ephemeral: gate2,
        gate3_persistent: gate3,
        gate4_nip05: gate4,
        gate5_nip11: gate5,
    })
}

/// Gate 2: ephemeral-profile cache starts empty on every open.
fn gate2_ephemeral_restart() -> CanaryResult<String> {
    let keys = Keys::generate();
    let cache1 = MemoryEventCache::default();
    let event = make_relay_event(&keys, "phase-e gate 2 ephemeral");
    cache1.admit(event, Timestamp::now()).map_err(error)?;
    let before = cache1.len().map_err(error)?;
    if before != 1 {
        return Err(CanaryError::new(format!(
            "gate2: expected 1 event before restart, got {before}"
        )));
    }
    // New instance simulates a process restart with ephemeral profile.
    let cache2 = MemoryEventCache::default();
    let after = cache2.len().map_err(error)?;
    if after != 0 {
        return Err(CanaryError::new(format!(
            "gate2: expected 0 events after ephemeral restart, got {after}"
        )));
    }
    Ok(format!(
        "admitted={before} after_restart={after} (ephemeral cache empty on restart)"
    ))
}

/// Gate 3: persistent-profile cache retains events across process restart.
fn gate3_persistent_restart(evidence_dir: &Path) -> CanaryResult<String> {
    let path = evidence_dir.join("gate3.redb");
    let keys = Keys::generate();
    let event = make_relay_event(&keys, "phase-e gate 3 persistent");
    let event_id = event.event().id;
    {
        let cache =
            RedbEventCache::open_bounded(&path, NonZeroUsize::new(100).expect("constant non-zero"))
                .map_err(error)?;
        cache.admit(event, Timestamp::now()).map_err(error)?;
    }
    // Reopen without any relay connection — event must survive.
    let cache2 = RedbEventCache::open(&path).map_err(error)?;
    let count = cache2.len().map_err(error)?;
    if count != 1 {
        return Err(CanaryError::new(format!(
            "gate3: expected 1 event after reopen, got {count}"
        )));
    }
    if cache2.event(event_id).map_err(error)?.is_none() {
        return Err(CanaryError::new("gate3: event absent by id after reopen"));
    }
    Ok(format!(
        "event={} survived_reopen=true count={count}",
        event_id.to_hex()
    ))
}

/// Gate 4: NIP-05 negative-cache absorbs second resolve (zero new HTTP calls).
async fn gate4_nip05_negative_cache() -> CanaryResult<String> {
    let cache = MemoryFetchCache::new();
    let fetcher = CountingFetcher::new(404, String::new());
    let ttl = Duration::from_secs(60);
    // First resolve: HTTP fetcher must be called exactly once → NotFound.
    let result1 = fava_fetch_cache::nip05::resolve("alice@127.0.0.1", &cache, &fetcher, ttl).await;
    if !matches!(result1, fava_fetch_cache::nip05::Nip05Result::NotFound) {
        return Err(CanaryError::new(
            "gate4: first resolve must return NotFound for 404 response",
        ));
    }
    let calls_after_first = fetcher.calls();
    if calls_after_first != 1 {
        return Err(CanaryError::new(format!(
            "gate4: expected 1 fetch call after first resolve, got {calls_after_first}"
        )));
    }
    // Second resolve: negative cache must absorb it (no new HTTP call).
    let result2 = fava_fetch_cache::nip05::resolve("alice@127.0.0.1", &cache, &fetcher, ttl).await;
    if !result2.is_negative_cached() {
        return Err(CanaryError::new(
            "gate4: second resolve must return NegativeCached",
        ));
    }
    let calls_after_second = fetcher.calls();
    if calls_after_second != 1 {
        return Err(CanaryError::new(format!(
            "gate4: expected 0 new calls on second resolve (cached), got {calls_after_second} total"
        )));
    }
    Ok(format!(
        "fetch_calls={calls_after_second} first=NotFound second=NegativeCached (negative cache absorbed re-fetch)"
    ))
}

/// Gate 5: NIP-11 stale-result with explicit freshness evidence after TTL expires.
async fn gate5_nip11_stale_result() -> CanaryResult<String> {
    let cache = MemoryFetchCache::new();
    let info_body = r#"{"name":"test-relay","description":"Phase E gate 5"}"#.to_owned();
    let fetcher = CountingFetcher::new(200, info_body);
    let relay_url = "ws://127.0.0.1:9876";
    let ttl = Duration::from_millis(1);
    // First fetch: cache miss → HTTP call → Fresh result.
    let result1 = fava_fetch_cache::nip11::fetch(relay_url, &cache, &fetcher, ttl).await;
    if result1.is_stale() {
        return Err(CanaryError::new("gate5: first result must not be stale"));
    }
    if result1.info_json().is_none() {
        return Err(CanaryError::new("gate5: first result must carry info_json"));
    }
    let calls_before_sleep = fetcher.calls();
    // Sleep past the 1ms TTL so the cache entry becomes stale.
    tokio::time::sleep(Duration::from_millis(50)).await;
    // Second fetch: stale cache hit → Stale result, no new HTTP call.
    let result2 = fava_fetch_cache::nip11::fetch(relay_url, &cache, &fetcher, ttl).await;
    let calls_after = fetcher.calls();
    if calls_after != calls_before_sleep {
        return Err(CanaryError::new(format!(
            "gate5: expected no new HTTP call after TTL expiry; calls before={calls_before_sleep} after={calls_after}"
        )));
    }
    if !result2.is_stale() {
        return Err(CanaryError::new(
            "gate5: second result must be Stale after TTL expired",
        ));
    }
    let age_ms = match &result2 {
        fava_fetch_cache::nip11::Nip11Result::Stale { age, .. } => age.age.as_millis(),
        _ => 0,
    };
    Ok(format!(
        "fetch_calls={calls_after} second_result=Stale age={age_ms}ms (stale evidence after {ttl:?} TTL)"
    ))
}

fn make_relay_event(keys: &Keys, content: &str) -> RelayEvent {
    let event = EventBuilder::new(Kind::TextNote, content)
        .custom_created_at(Timestamp::now())
        .finalize(keys)
        .expect("signed event");
    let session = RelaySessionKey {
        relay: RelayUrl::parse("ws://127.0.0.1:7777").expect("valid relay url"),
        access: RelayAccess::Public,
    };
    RelayEvent::new(event, session, Timestamp::now())
}

struct CountingFetcher {
    calls: AtomicU64,
    status: u16,
    body: String,
}

impl CountingFetcher {
    fn new(status: u16, body: String) -> Self {
        Self {
            calls: AtomicU64::new(0),
            status,
            body,
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl HttpFetcher for CountingFetcher {
    fn get<'a>(&'a self, _url: &'a str) -> FetchFuture<'a> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let status = self.status;
        let body = self.body.clone();
        Box::pin(async move { Ok::<HttpResponse, String>(HttpResponse { status, body }) })
    }
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
