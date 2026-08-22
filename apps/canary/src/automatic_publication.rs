//! Real-relay M6 automatic routing and partial-delivery scenarios.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use fava::{EventBuilder, EventValue, Query, ReceiptOutcome};
use fava_event_cache_memory::MemoryEventCache;
use fava_query::QuerySource;
use fava_router_app_relays::AppRelayRouter;
use fava_router_fallback_relays::FallbackRelayRouter;
use fava_router_hints::HintRouter;
use fava_router_outbox::OutboxRouter;
use fava_routing::Router;
use fava_signer::Signer;
use fava_signer_local::LocalSigner;
use fava_state::{RelayUrl, Timestamp};
use fava_write::{Event, Kind, Tag, WriteIntent, WriteRouting};
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use serde_json::{Value, json};

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::automatic_support::{error, publication_fava, query_fava, start_relays};
use crate::publication_support::{finish, wait_record, wait_terminal, wait_wire, wire_count};
use crate::{CanaryError, CanaryResult, SmokeOptions, deterministic_keys, wire};

/// Run one complete M6 scenario through real relay processes.
///
/// # Errors
///
/// Returns an exact routing, publication, relay, or evidence failure.
pub async fn run_automatic_publication_scenario(
    id: &str,
    options: SmokeOptions,
) -> CanaryResult<PathBuf> {
    let count = match id {
        "async-recipient-routing" => 5,
        "hint-routing" | "route-preview-parity" => 1,
        "app-relay-versus-fallback-profile" => 2,
        _ => return Err(CanaryError::new(format!("unknown M6 scenario: {id}"))),
    };
    let mut artifacts = RunArtifacts::create(&options.runs_directory, id, &options.seed)?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": id, "seed": options.seed }),
    )?;
    let started = unix_ms()?;
    let (version, mut relays, mut facts) = start_relays(&mut artifacts, &options, count).await?;
    let result = match id {
        "async-recipient-routing" => async_recipients(&artifacts, &options.seed, &relays).await,
        "hint-routing" => hint_routing(&artifacts, &options.seed, &relays).await,
        "route-preview-parity" => preview_parity(&artifacts, &options.seed, &relays).await,
        "app-relay-versus-fallback-profile" => {
            profile_selection(&artifacts, &options.seed, &relays).await
        }
        _ => unreachable!("validated scenario"),
    };
    for relay in relays.drain(..) {
        facts.push(relay.stop().await?);
    }
    let completed = result?;
    finish(
        artifacts,
        id,
        &options,
        started,
        &version,
        &facts,
        &completed.event_id,
        completed.receipt_id,
        &completed.details,
    )
}

struct Completed {
    event_id: String,
    receipt_id: u64,
    details: Value,
}

async fn async_recipients(
    artifacts: &RunArtifacts,
    seed: &str,
    relays: &[crate::automatic_support::LabRelay],
) -> CanaryResult<Completed> {
    let author = deterministic_keys(&format!("{seed}-author"))?;
    let recipients = [
        deterministic_keys(&format!("{seed}-recipient-a"))?,
        deterministic_keys(&format!("{seed}-recipient-b"))?,
        deterministic_keys(&format!("{seed}-recipient-c"))?,
    ];
    let urls = relay_urls(relays)?;
    let cache = Arc::new(MemoryEventCache::default());
    let queries: Arc<dyn QuerySource> = Arc::new(query_fava(Arc::clone(&cache))?);
    let outbox = Arc::new(OutboxRouter::new("nip65", [urls[4].clone()], queries).map_err(error)?);
    outbox
        .remember(&EventValue::Signed(relay_list(
            &author,
            None,
            Some(&urls[1]),
        )?))
        .map_err(error)?;
    for (keys, relay) in recipients.iter().zip(&urls[1..3]) {
        outbox
            .remember(&EventValue::Signed(relay_list(keys, Some(relay), None)?))
            .map_err(error)?;
    }
    let routers: Vec<Arc<dyn Router>> = vec![
        outbox,
        Arc::new(AppRelayRouter::new("app-relays", [urls[0].clone()])),
    ];
    let signer: Arc<dyn Signer> = Arc::new(LocalSigner::new(author.clone()));
    let fava = publication_fava(
        cache,
        &artifacts.root().join("children/async-routing.redb"),
        routers,
        Some(signer),
    )?;
    let event = tagged_event(&author, &recipients, format!("Fava M6 async {seed}"))?;
    let event_id = event.id.expect("checked event id");
    let intent = WriteIntent::event(event.clone(), WriteRouting::Automatic).map_err(error)?;
    let preview = fava.preview_write_routes(&intent).map_err(error)?;
    if preview.destinations.len() != 3 || preview.settled {
        return Err(CanaryError::new(format!(
            "initial preview was not partial: {preview:?}"
        )));
    }
    if !fava.open_receipts().map_err(error)?.is_empty() {
        return Err(CanaryError::new("route preview performed publication work"));
    }
    for relay in relays {
        if wire_count(&relay.log, "EVENT")? != 0 {
            return Err(CanaryError::new("route preview performed publication work"));
        }
    }
    let accepted = fava.publish(event).map_err(error)?;
    wait_wire(&relays[0].log, "EVENT", 1).await?;
    wait_wire(&relays[1].log, "EVENT", 1).await?;
    wait_wire(&relays[2].log, "EVENT", 1).await?;
    wait_wire(&relays[4].log, "REQ", 1).await?;
    if wire_count(&relays[3].log, "EVENT")? != 0 {
        return Err(CanaryError::new(
            "unknown recipient relay received an early EVENT",
        ));
    }
    let first_handoff = unix_ms()?;
    let later_list = relay_list(&recipients[2], Some(&urls[3]), None)?;
    wire::publish(&relays[4].url, &later_list).await?;
    let discovery_seeded = unix_ms()?;
    wait_wire(&relays[3].log, "EVENT", 1).await?;
    let receipt = wait_terminal(&accepted).await?;
    for relay in &relays[..4] {
        if wire_count(&relay.log, "EVENT")? != 1 {
            return Err(CanaryError::new("route expansion duplicated an EVENT send"));
        }
    }
    if first_handoff > discovery_seeded
        || receipt.receipt_id != accepted.receipt_id()
        || receipt.destinations().len() != 4
        || receipt.outcome != ReceiptOutcome::Complete
    {
        return Err(CanaryError::new(
            "partial route did not expand under one receipt",
        ));
    }
    Ok(Completed {
        event_id: event_id.to_hex(),
        receipt_id: receipt.receipt_id.as_u64(),
        details: json!({
            "first_handoff_before_third_list": first_handoff <= discovery_seeded,
            "initial_destinations": preview.destinations.len(),
            "final_destinations": receipt.destinations().len(),
            "route_revision": receipt.route_revision,
            "duplicate_sends": false,
        }),
    })
}

async fn hint_routing(
    artifacts: &RunArtifacts,
    seed: &str,
    relays: &[crate::automatic_support::LabRelay],
) -> CanaryResult<Completed> {
    let target_keys = deterministic_keys(&format!("{seed}-target"))?;
    let author = deterministic_keys(&format!("{seed}-reply"))?;
    let target =
        nostr::event::EventBuilder::new(Kind::TextNote, "M6 hint target").finalize(&target_keys)?;
    wire::publish(&relays[0].url, &target).await?;
    let cache = Arc::new(MemoryEventCache::default());
    let hint = Arc::new(HintRouter::new("hints"));
    let routers: Vec<Arc<dyn Router>> = vec![hint.clone()];
    let signer: Arc<dyn Signer> = Arc::new(LocalSigner::new(author.clone()));
    let fava = publication_fava(
        cache,
        &artifacts.root().join("children/hints.redb"),
        routers,
        Some(signer),
    )?;
    let relay = RelayUrl::parse(&relays[0].url).map_err(error)?;
    let mut observation = fava
        .observe(
            Query::events()
                .ids([target.id])
                .from_relays([relay])
                .map_err(error)?,
        )
        .await
        .map_err(error)?;
    wait_record(&mut observation, target.id, 1).await?;
    let record = observation
        .current()
        .events
        .first()
        .cloned()
        .ok_or_else(|| CanaryError::new("target event record missing"))?;
    hint.remember(&record);
    observation.close();
    let reply = EventBuilder::new(author.public_key(), Kind::TextNote)
        .content(format!("Fava M6 reply {seed}"))
        .tag(Tag::parse(["e", &target.id.to_hex(), &relays[0].url, "reply"]).map_err(error)?)
        .build()
        .map_err(error)?;
    let reply_id = reply.id.expect("checked event id");
    let accepted = fava.publish(reply).map_err(error)?;
    let receipt = wait_terminal(&accepted).await?;
    wait_wire(&relays[0].log, "EVENT", 2).await?;
    let witness = wire::query_exact(&relays[0].url, reply_id, "m6-hint-reply").await?;
    if !witness.found_event || receipt.destinations().len() != 1 {
        return Err(CanaryError::new(
            "hint router did not select the evidenced relay",
        ));
    }
    Ok(Completed {
        event_id: reply_id.to_hex(),
        receipt_id: receipt.receipt_id.as_u64(),
        details: json!({ "hinted_relay": relays[0].url, "relay_evidence_count": 1 }),
    })
}

async fn preview_parity(
    artifacts: &RunArtifacts,
    seed: &str,
    relays: &[crate::automatic_support::LabRelay],
) -> CanaryResult<Completed> {
    let keys = deterministic_keys(seed)?;
    let relay = RelayUrl::parse(&relays[0].url).map_err(error)?;
    let routers: Vec<Arc<dyn Router>> = vec![Arc::new(AppRelayRouter::new("app-relays", [relay]))];
    let signer: Arc<dyn Signer> = Arc::new(LocalSigner::new(keys.clone()));
    let fava = publication_fava(
        Arc::new(MemoryEventCache::default()),
        &artifacts.root().join("children/preview.redb"),
        routers,
        Some(signer),
    )?;
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .content(format!("Fava M6 preview {seed}"))
        .build()
        .map_err(error)?;
    let event_id = event.id.expect("checked event id");
    let intent = WriteIntent::event(event.clone(), WriteRouting::Automatic).map_err(error)?;
    let preview = fava.preview_write_routes(&intent).map_err(error)?;
    if !fava.open_receipts().map_err(error)?.is_empty() || wire_count(&relays[0].log, "EVENT")? != 0
    {
        return Err(CanaryError::new("preview created custody or relay work"));
    }
    let accepted = fava.publish(event).map_err(error)?;
    let receipt = wait_terminal(&accepted).await?;
    if receipt.desired_destinations != preview.destinations.keys().cloned().collect() {
        return Err(CanaryError::new("preview and initial real route differed"));
    }
    Ok(Completed {
        event_id: event_id.to_hex(),
        receipt_id: receipt.receipt_id.as_u64(),
        details: json!({ "preview_destinations": preview.destinations.len(), "parity": true }),
    })
}

async fn profile_selection(
    artifacts: &RunArtifacts,
    seed: &str,
    relays: &[crate::automatic_support::LabRelay],
) -> CanaryResult<Completed> {
    let keys = deterministic_keys(seed)?;
    let event =
        nostr::event::EventBuilder::new(Kind::TextNote, "Fava M6 profiles").finalize(&keys)?;
    let urls = relay_urls(relays)?;
    let app_routers: Vec<Arc<dyn Router>> = vec![Arc::new(
        AppRelayRouter::new("app-relays", [urls[0].clone()]).reads(false),
    )];
    let fallback_routers: Vec<Arc<dyn Router>> = vec![Arc::new(
        FallbackRelayRouter::new(
            "fallback",
            [urls[1].clone()],
            NonZeroUsize::new(1).expect("non-zero"),
        )
        .reads(false),
    )];
    let app = publication_fava(
        Arc::new(MemoryEventCache::default()),
        &artifacts.root().join("children/app-profile.redb"),
        app_routers,
        None,
    )?;
    let fallback = publication_fava(
        Arc::new(MemoryEventCache::default()),
        &artifacts.root().join("children/fallback-profile.redb"),
        fallback_routers,
        None,
    )?;
    let app_intent =
        WriteIntent::presigned(event.clone(), WriteRouting::Automatic).map_err(error)?;
    let fallback_intent =
        WriteIntent::presigned(event.clone(), WriteRouting::Automatic).map_err(error)?;
    let app_plan = app.preview_write_routes(&app_intent).map_err(error)?;
    let fallback_plan = fallback
        .preview_write_routes(&fallback_intent)
        .map_err(error)?;
    if app_plan
        .destinations
        .keys()
        .eq(fallback_plan.destinations.keys())
    {
        return Err(CanaryError::new(
            "app and fallback profiles produced one plan",
        ));
    }
    let app_write = app.publish(event.clone()).map_err(error)?;
    let fallback_write = fallback.publish(event.clone()).map_err(error)?;
    let app_receipt = wait_terminal(&app_write).await?;
    let fallback_receipt = wait_terminal(&fallback_write).await?;
    wait_wire(&relays[0].log, "EVENT", 1).await?;
    wait_wire(&relays[1].log, "EVENT", 1).await?;
    Ok(Completed {
        event_id: event.id.to_hex(),
        receipt_id: app_receipt.receipt_id.as_u64(),
        details: json!({
            "app_profile_destinations": app_receipt.destinations().len(),
            "fallback_profile_destinations": fallback_receipt.destinations().len(),
            "same_event": true,
        }),
    })
}

fn tagged_event(
    author: &Keys,
    recipients: &[Keys],
    content: String,
) -> CanaryResult<fava_write::UnsignedEvent> {
    let mut builder = EventBuilder::new(author.public_key(), Kind::TextNote).content(content);
    for recipient in recipients {
        builder = builder.tag(Tag::parse(["p", &recipient.public_key().to_hex()]).map_err(error)?);
    }
    builder.build().map_err(error)
}

fn relay_list(
    keys: &Keys,
    read: Option<&RelayUrl>,
    write: Option<&RelayUrl>,
) -> CanaryResult<Event> {
    let mut builder =
        EventBuilder::new(keys.public_key(), Kind::from(10_002_u16)).created_at(Timestamp::now());
    if let Some(relay) = read {
        builder = builder.tag(Tag::parse(["r", relay.as_str(), "read"]).map_err(error)?);
    }
    if let Some(relay) = write {
        builder = builder.tag(Tag::parse(["r", relay.as_str(), "write"]).map_err(error)?);
    }
    builder
        .build()
        .map_err(error)?
        .finalize(keys)
        .map_err(error)
}

fn relay_urls(relays: &[crate::automatic_support::LabRelay]) -> CanaryResult<Vec<RelayUrl>> {
    relays
        .iter()
        .map(|relay| RelayUrl::parse(&relay.url).map_err(error))
        .collect()
}
