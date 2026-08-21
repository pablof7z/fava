//! Deterministic public-Fava semantic-write canaries.

use std::path::PathBuf;
use std::sync::Arc;

use fava::{EventValue, Kind, MaterializationId, Query, ReplaceableEventMaterializer, Timestamp};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_signer::Signer;
use fava_state::CachedEvent;
use fava_write::EventId;
use nostr::event::{EventBuilder, FinalizeEvent, Tag};
use serde_json::{Value, json};

use crate::artifacts::RunArtifacts;
#[path = "semantic_delivery_support.rs"]
mod semantic_delivery_support;
use crate::semantic_failure::write_failure_bundle;
use crate::semantic_n_plus_one;
use crate::semantic_write_support::{
    DeterministicSigner, GateSigner, RecordingPublisher, assembly, attempt_evidence,
    deterministic_finalize, explicit, finish, next_sign, published_event, relay_evidence,
    target_count, wait_completion, wait_query_event, wait_terminal,
};
use crate::{CanaryError, CanaryResult, SmokeOptions, deterministic_keys};
use semantic_delivery_support::{
    GatePublisher, exact_receipt, next_delivery, require_exact_attempt_progress,
    require_exact_terminal_progress, require_generation_two_pending,
};

pub(crate) fn has_executor(id: &str) -> bool {
    matches!(
        id,
        "replaceable-edit-first-value"
            | "replaceable-edit-rematerialization"
            | "replaceable-edit-inverse"
            | "protocol-crate-n-plus-one"
    )
}

/// Run one deterministic M7 semantic-write scenario through public Fava.
///
/// # Errors
///
/// Returns an exact composition, lifecycle, external-process, or evidence failure.
pub async fn run_semantic_write_scenario(id: &str, options: SmokeOptions) -> CanaryResult<PathBuf> {
    if !has_executor(id) {
        return Err(CanaryError::new(format!("unknown M7 scenario: {id}")));
    }
    let mut artifacts = RunArtifacts::create(&options.runs_directory, id, &options.seed)?;
    artifacts.record(
        "scenario_started",
        json!({
            "scenario": id,
            "seed_sha256": crate::semantic_write_support::seed_hash(&options.seed),
        }),
    )?;
    let outcome = match id {
        "replaceable-edit-first-value" => first_value(&options.seed).await,
        "replaceable-edit-rematerialization" => rematerialization(&options.seed).await,
        "replaceable-edit-inverse" => inverse(&options.seed).await,
        "protocol-crate-n-plus-one" => semantic_n_plus_one::execute(&options.seed).await,
        _ => unreachable!("executor checked above"),
    };
    match outcome {
        Ok(details) => finish(artifacts, id, &options, &details),
        Err(failure) => {
            let message = failure.to_string();
            let root = write_failure_bundle(artifacts, id, &options, &message)?;
            Err(CanaryError::new(format!(
                "{message}; durable evidence: {}",
                root.display()
            )))
        }
    }
}

async fn first_value(seed: &str) -> CanaryResult<Value> {
    let keys = deterministic_keys(&format!("{seed}-actor"))?;
    let target = deterministic_keys(&format!("{seed}-target"))?.public_key();
    let cache = Arc::new(MemoryEventCache::default());
    let publisher = Arc::new(RecordingPublisher::default());
    let signer: Arc<dyn Signer> = Arc::new(DeterministicSigner::new(keys.clone()));
    let (fava, _completions) = assembly(
        Arc::clone(&cache),
        signer,
        selected_materializers(),
        Arc::clone(&publisher),
    )?;
    let mut query = fava
        .observe(
            Query::events()
                .authors([keys.public_key()])
                .kind(Kind::ContactList)
                .cache_only(),
        )
        .await
        .map_err(error)?;
    let intent = fava_nip02::follow(target).map_err(error)?;
    let accepted = fava
        .publish(explicit(intent, keys.public_key())?)
        .map_err(error)?;
    let receipt = wait_terminal(&fava, accepted.receipt_id).await?;
    wait_query_event(&mut query, receipt.current.id()).await?;
    let attempts = publisher.attempts();
    let attempt = attempts
        .first()
        .ok_or_else(|| CanaryError::new("first-value publication attempt missing"))?;
    let attempt = attempt_evidence(&accepted, &receipt, attempt)?;
    let event = published_event(&receipt)?;
    let created_at = event.created_at.as_secs();
    if attempts.len() != 1
        || receipt.current.publication.materialization_source.is_some()
        || receipt.current.publication.materialization_id != MaterializationId::from_u64(1)
        || target_count(&event, "p", &target.to_hex()) != 1
    {
        return Err(CanaryError::new("first-value lifecycle facts diverged"));
    }
    Ok(json!({
        "event_id": receipt.current.id().to_hex(),
        "write_id": accepted.write_id.as_u64(),
        "receipt_id": accepted.receipt_id.as_u64(),
        "materialization_id": 1,
        "source_id": Value::Null,
        "route": receipt.desired_destinations.iter().map(|key| key.relay.to_string()).collect::<Vec<_>>(),
        "publisher_attempts": attempts.len(),
        "query_events": query.current().events.len(),
        "cache_events": cache.len().map_err(error)?,
        "attempt": attempt,
        "created_at": created_at,
        "event_bytes": serde_json::to_string(&event).map_err(error)?,
    }))
}

// Keeping the held generation-one lane and exact generation-two transitions in
// one ordered body makes the causal barrier reviewable rather than implicit.
#[allow(clippy::too_many_lines)]
async fn rematerialization(seed: &str) -> CanaryResult<Value> {
    let keys = deterministic_keys(&format!("{seed}-actor"))?;
    let bob = deterministic_keys(&format!("{seed}-bob"))?.public_key();
    let carol = deterministic_keys(&format!("{seed}-carol"))?.public_key();
    let source_one = contact_source(&keys, &[], 10)?;
    let source_two = contact_source(&keys, &[carol], 20)?;
    let source_one_bob_count = target_count(&source_one, "p", &bob.to_hex());
    let source_two_bob_count = target_count(&source_two, "p", &bob.to_hex());
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .admit(
            CachedEvent::new(source_one, relay_evidence()),
            Timestamp::from(11),
        )
        .map_err(error)?;
    let (gate, mut requests) = GateSigner::new(keys.public_key());
    let signer: Arc<dyn Signer> = Arc::new(gate);
    let (publisher, mut deliveries) = GatePublisher::new();
    let publisher = Arc::new(publisher);
    let (fava, mut completions) = assembly(
        Arc::clone(&cache),
        signer,
        selected_materializers(),
        Arc::clone(&publisher),
    )?;
    let accepted = fava
        .publish(explicit(
            fava_nip02::follow(bob).map_err(error)?,
            keys.public_key(),
        )?)
        .map_err(error)?;
    let first = next_sign(&mut requests).await?;
    let first_created_at = first.event.created_at.as_secs();
    let first_event = deterministic_finalize(first.event.clone(), &keys).map_err(error)?;
    let first_id = first_event.id;
    first.complete(first_event)?;
    let first_installed = wait_completion(&mut completions, 1).await?;
    require_completion(&first_installed, &accepted, 1, first_id, true, "first")?;
    let first_delivery = next_delivery(&mut deliveries).await?;
    let first_receipt = exact_receipt(&fava, accepted.receipt_id)?;
    let first_attempt = attempt_evidence(&accepted, &first_receipt, &first_delivery.attempt)?;
    let first_delivery_materialization_id = first_delivery.attempt.materialization_id.as_u64();
    cache
        .admit(
            CachedEvent::new(source_two.clone(), relay_evidence()),
            Timestamp::from(21),
        )
        .map_err(error)?;
    let current = next_sign(&mut requests).await?;
    let current_created_at = current.event.created_at.as_secs();
    if current_created_at <= first_created_at {
        return Err(CanaryError::new(
            "rematerialization timestamp did not advance monotonically",
        ));
    }
    let current_event = deterministic_finalize(current.event.clone(), &keys).map_err(error)?;
    let current_id = current_event.id;
    current.complete(current_event)?;
    let installed = wait_completion(&mut completions, 2).await?;
    require_completion(&installed, &accepted, 2, current_id, true, "current")?;
    let generation_two_pending = exact_receipt(&fava, accepted.receipt_id)?;
    require_generation_two_pending(&generation_two_pending, current_id)?;
    first_delivery.complete(fava_publisher::PublishOutcome::Acknowledged {
        message: "retired generation acknowledgement".to_owned(),
    })?;
    // The sole session cannot open generation two until the held generation-one
    // lane has processed its stale outcome and left the active-lane map.
    let current_delivery = next_delivery(&mut deliveries).await?;
    let generation_two_attempting = exact_receipt(&fava, accepted.receipt_id)?;
    let generation_two_exact_after_retired_delivery = require_exact_attempt_progress(
        &generation_two_pending,
        &generation_two_attempting,
        &current_delivery.attempt,
    )?;
    let current_attempt = attempt_evidence(
        &accepted,
        &generation_two_attempting,
        &current_delivery.attempt,
    )?;
    current_delivery.complete(fava_publisher::PublishOutcome::Acknowledged {
        message: "current generation acknowledgement".to_owned(),
    })?;
    let receipt = wait_terminal(&fava, accepted.receipt_id).await?;
    require_exact_terminal_progress(&generation_two_attempting, &receipt)?;
    let event = published_event(&receipt)?;
    let current_bob_count = target_count(&event, "p", &bob.to_hex());
    let current_carol_count = target_count(&event, "p", &carol.to_hex());
    let preserved = current_bob_count == 1
        && current_carol_count == 1
        && event
            .tags
            .iter()
            .any(|tag| tag.as_slice() == ["x", "unrelated", "bytes"]);
    if receipt.current.publication.materialization_id != MaterializationId::from_u64(2)
        || receipt.current.publication.materialization_source != Some(source_two.id)
        || receipt.current.publication.retired_materializations.len() != 1
        || source_one_bob_count != 0
        || source_two_bob_count != 0
        || !preserved
    {
        return Err(CanaryError::new(
            "rematerialization lifecycle facts diverged",
        ));
    }
    let timestamp_exhaustion_preserved_current =
        prove_timestamp_exhaustion(&format!("{seed}-exhaustion")).await?;
    Ok(json!({
        "event_id": event.id.to_hex(),
        "write_id": accepted.write_id.as_u64(),
        "receipt_id": accepted.receipt_id.as_u64(),
        "first_materialization_id": 1,
        "current_materialization_id": 2,
        "source_id": source_two.id.to_hex(),
        "retired_materializations": receipt.current.publication.retired_materializations.len(),
        "source_one_bob_count": source_one_bob_count,
        "source_two_bob_count": source_two_bob_count,
        "current_bob_count": current_bob_count,
        "current_carol_count": current_carol_count,
        "publisher_attempts": 2,
        "preserved_bob_carol_unrelated": preserved,
        "first_delivery_materialization_id": first_delivery_materialization_id,
        "current_delivery_materialization_id": receipt.current.publication.materialization_id.as_u64(),
        "retired_delivery_completion_processed": true,
        "retired_delivery_installed": false,
        "generation_two_unchanged_after_retired_delivery": generation_two_exact_after_retired_delivery,
        "first_created_at": first_created_at,
        "current_created_at": current_created_at,
        "timestamp_exhaustion_preserved_current": timestamp_exhaustion_preserved_current,
        "first_attempt": first_attempt,
        "attempt": current_attempt,
        "event_bytes": serde_json::to_string(&event).map_err(error)?,
    }))
}

async fn prove_timestamp_exhaustion(seed: &str) -> CanaryResult<bool> {
    let keys = deterministic_keys(seed)?;
    let target = deterministic_keys(&format!("{seed}-target"))?.public_key();
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .admit(
            CachedEvent::new(contact_source(&keys, &[], u64::MAX - 1)?, relay_evidence()),
            Timestamp::from(u64::MAX - 1),
        )
        .map_err(error)?;
    let publisher = Arc::new(RecordingPublisher::default());
    let (gate, mut requests) = GateSigner::new(keys.public_key());
    let signer: Arc<dyn Signer> = Arc::new(gate);
    let (fava, _completions) = assembly(
        Arc::clone(&cache),
        signer,
        selected_materializers(),
        Arc::clone(&publisher),
    )?;
    let accepted = fava
        .publish(explicit(
            fava_nip02::follow(target).map_err(error)?,
            keys.public_key(),
        )?)
        .map_err(error)?;
    let _pending = next_sign(&mut requests).await?;
    let source = contact_source(&keys, &[], u64::MAX)?;
    cache
        .admit(
            CachedEvent::new(source, relay_evidence()),
            Timestamp::from(u64::MAX),
        )
        .map_err(error)?;
    let exhausted = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let receipt = fava
                .receipt(accepted.receipt_id)
                .map_err(error)?
                .ok_or_else(|| CanaryError::new("timestamp-exhaustion receipt disappeared"))?;
            if receipt
                .current
                .publication
                .materialization_failure
                .as_deref()
                .is_some_and(|reason| reason.contains("timestamp exhausted"))
            {
                return Ok::<fava::Receipt, CanaryError>(receipt);
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .map_err(|_| CanaryError::new("timed out awaiting timestamp-exhaustion evidence"))??;
    let preserved = exhausted.current.id() == accepted.current.id()
        && exhausted.current.publication.materialization_id
            == accepted.current.publication.materialization_id
        && publisher.attempts().is_empty();
    if !preserved {
        return Err(CanaryError::new(
            "timestamp exhaustion changed current state or publication evidence",
        ));
    }
    Ok(true)
}

fn contact_source(
    keys: &nostr::key::Keys,
    participants: &[fava::PublicKey],
    created_at: u64,
) -> CanaryResult<fava::Event> {
    let mut tags = participants
        .iter()
        .map(|participant| Tag::parse(["p", &participant.to_hex()]).map_err(error))
        .collect::<CanaryResult<Vec<_>>>()?;
    tags.push(Tag::parse(["x", "unrelated", "bytes"]).map_err(error)?);
    EventBuilder::new(Kind::ContactList, "opaque")
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .map_err(error)
}

fn require_completion(
    completion: &crate::semantic_write_store::CompletionAck,
    accepted: &fava_write_store::AcceptedWrite,
    materialization_id: u64,
    event_id: EventId,
    installed: bool,
    label: &str,
) -> CanaryResult<()> {
    if completion.write_id != accepted.write_id
        || completion.receipt_id != accepted.receipt_id
        || completion.materialization_id != MaterializationId::from_u64(materialization_id)
        || completion.event_id != event_id
        || completion.installed != installed
    {
        return Err(CanaryError::new(format!(
            "{label} completion acknowledgement diverged"
        )));
    }
    Ok(())
}

async fn inverse(seed: &str) -> CanaryResult<Value> {
    let keys = deterministic_keys(&format!("{seed}-actor"))?;
    let bob = deterministic_keys(&format!("{seed}-bob"))?.public_key();
    let carol = deterministic_keys(&format!("{seed}-carol"))?.public_key();
    let bookmark_a = EventId::from_byte_array([41; 32]);
    let bookmark_b = EventId::from_byte_array([42; 32]);
    let cache = Arc::new(MemoryEventCache::default());
    let publisher = Arc::new(RecordingPublisher::default());
    let signer: Arc<dyn Signer> = Arc::new(DeterministicSigner::new(keys.clone()));
    let (fava, _completions) = assembly(
        Arc::clone(&cache),
        signer,
        selected_materializers(),
        Arc::clone(&publisher),
    )?;
    let actor = keys.public_key();
    let edits = vec![
        fava_nip02::follow(bob),
        fava_nip02::follow(carol),
        fava_nip02::unfollow(bob),
        fava_nip02::unfollow(carol),
        fava_nip02::unfollow(bob),
        fava_bookmarks::bookmark_event(bookmark_a),
        fava_bookmarks::bookmark_event(bookmark_b),
        fava_bookmarks::unbookmark_event(bookmark_a),
        fava_bookmarks::unbookmark_event(bookmark_b),
        fava_bookmarks::unbookmark_event(bookmark_a),
    ];
    let mut receipts = Vec::with_capacity(edits.len());
    let mut attempts = Vec::with_capacity(edits.len());
    for edit in edits {
        let accepted = fava
            .publish(explicit(edit.map_err(error)?, actor)?)
            .map_err(error)?;
        let receipt = wait_terminal(&fava, accepted.receipt_id).await?;
        let event = published_event(&receipt)?;
        let publication_attempt = publisher
            .attempts()
            .last()
            .cloned()
            .ok_or_else(|| CanaryError::new("inverse publication attempt missing"))?;
        attempts.push(attempt_evidence(&accepted, &receipt, &publication_attempt)?);
        cache
            .admit(
                CachedEvent::new(event, relay_evidence()),
                Timestamp::from(u64::MAX),
            )
            .map_err(error)?;
        receipts.push(receipt.receipt_id.as_u64());
    }
    let nip02 = current_event(&fava, actor, Kind::ContactList).await?;
    let bookmarks = current_event(&fava, actor, Kind::Custom(10_003)).await?;
    let nip02_targets =
        target_count(&nip02, "p", &bob.to_hex()) + target_count(&nip02, "p", &carol.to_hex());
    let bookmark_targets = target_count(&bookmarks, "e", &bookmark_a.to_hex())
        + target_count(&bookmarks, "e", &bookmark_b.to_hex());
    if nip02_targets != 0 || bookmark_targets != 0 || publisher.attempts().len() != 10 {
        return Err(CanaryError::new(
            "inverse lifecycle did not return to empty state",
        ));
    }
    Ok(json!({
        "nip02_event_id": nip02.id.to_hex(),
        "bookmark_event_id": bookmarks.id.to_hex(),
        "receipt_ids": receipts,
        "nip02_final_targets": nip02_targets,
        "bookmark_final_targets": bookmark_targets,
        "operations": 10,
        "empty_and_adjacent": true,
        "publisher_attempts": publisher.attempts().len(),
        "attempts": attempts,
    }))
}

async fn current_event(
    fava: &fava::Fava,
    actor: fava::PublicKey,
    kind: Kind,
) -> CanaryResult<fava::Event> {
    let observation = fava
        .observe(Query::events().authors([actor]).kind(kind).cache_only())
        .await
        .map_err(error)?;
    let snapshot = observation.current();
    let record = snapshot
        .events
        .first()
        .ok_or_else(|| CanaryError::new("final public query returned no event"))?;
    match &record.event {
        EventValue::Signed(event) => Ok(event.clone()),
        EventValue::Unsigned(_) => Err(CanaryError::new(
            "final public query returned unsigned state",
        )),
    }
}

fn selected_materializers() -> Vec<Arc<dyn ReplaceableEventMaterializer>> {
    vec![fava_nip02::materializer(), fava_bookmarks::materializer()]
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}

#[cfg(test)]
#[path = "semantic_writes_tests.rs"]
mod tests;
