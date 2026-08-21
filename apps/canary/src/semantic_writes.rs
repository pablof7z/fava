//! Deterministic public-Fava semantic-write canaries.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use fava::{EventValue, Kind, MaterializationId, Query, ReplaceableEventMaterializer, Timestamp};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_signer::Signer;
use fava_signer_local::LocalSigner;
use fava_state::CachedEvent;
use fava_write::EventId;
use nostr::event::{EventBuilder, FinalizeEvent, Tag};
use serde_json::{Value, json};

use crate::artifacts::RunArtifacts;
use crate::semantic_write_support::{
    GateSigner, RecordingPublisher, assembly, explicit, finish, next_sign, published_event,
    relay_evidence, target_count, wait_query_event, wait_terminal,
};
use crate::{CanaryError, CanaryResult, SmokeOptions, deterministic_keys, repository_root};

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
    let details = match id {
        "replaceable-edit-first-value" => first_value(&options.seed).await?,
        "replaceable-edit-rematerialization" => rematerialization(&options.seed).await?,
        "replaceable-edit-inverse" => inverse(&options.seed).await?,
        "protocol-crate-n-plus-one" => n_plus_one()?,
        _ => unreachable!("executor checked above"),
    };
    finish(artifacts, id, &options, &details)
}

async fn first_value(seed: &str) -> CanaryResult<Value> {
    let keys = deterministic_keys(&format!("{seed}-actor"))?;
    let target = deterministic_keys(&format!("{seed}-target"))?.public_key();
    let cache = Arc::new(MemoryEventCache::default());
    let publisher = Arc::new(RecordingPublisher::default());
    let signer: Arc<dyn Signer> = Arc::new(LocalSigner::new(keys.clone()));
    let fava = assembly(
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
    let intent = fava_nip02::follow(keys.public_key(), target).map_err(error)?;
    let accepted = fava.publish(explicit(intent)?).map_err(error)?;
    let receipt = wait_terminal(&fava, accepted.receipt_id).await?;
    wait_query_event(&mut query, receipt.current.id()).await?;
    let attempts = publisher.attempts();
    if attempts.len() != 1
        || receipt.current.publication.materialization_source.is_some()
        || receipt.current.publication.materialization_id != MaterializationId::from_u64(1)
        || target_count(&published_event(&receipt)?, "p", &target.to_hex()) != 1
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
    }))
}

async fn rematerialization(seed: &str) -> CanaryResult<Value> {
    let keys = deterministic_keys(&format!("{seed}-actor"))?;
    let bob = deterministic_keys(&format!("{seed}-bob"))?.public_key();
    let carol = deterministic_keys(&format!("{seed}-carol"))?.public_key();
    let source_one = EventBuilder::new(Kind::ContactList, "opaque")
        .tags(vec![
            Tag::parse(["p", &bob.to_hex()]).map_err(error)?,
            Tag::parse(["x", "unrelated", "bytes"]).map_err(error)?,
        ])
        .custom_created_at(Timestamp::from(10))
        .finalize(&keys)?;
    let source_two = EventBuilder::new(Kind::ContactList, "opaque")
        .tags(vec![
            Tag::parse(["p", &bob.to_hex()]).map_err(error)?,
            Tag::parse(["p", &carol.to_hex()]).map_err(error)?,
            Tag::parse(["x", "unrelated", "bytes"]).map_err(error)?,
        ])
        .custom_created_at(Timestamp::from(20))
        .finalize(&keys)?;
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .admit(
            CachedEvent::new(source_one, relay_evidence()),
            Timestamp::from(11),
        )
        .map_err(error)?;
    let (gate, mut requests) = GateSigner::new(keys.public_key());
    let signer: Arc<dyn Signer> = Arc::new(gate);
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = assembly(
        Arc::clone(&cache),
        signer,
        selected_materializers(),
        Arc::clone(&publisher),
    )?;
    let accepted = fava
        .publish(explicit(
            fava_nip02::follow(keys.public_key(), bob).map_err(error)?,
        )?)
        .map_err(error)?;
    let first = next_sign(&mut requests).await?;
    cache
        .admit(
            CachedEvent::new(source_two.clone(), relay_evidence()),
            Timestamp::from(21),
        )
        .map_err(error)?;
    let current = next_sign(&mut requests).await?;
    let first_event = first.event.clone().finalize(&keys).map_err(error)?;
    first.complete(first_event)?;
    let after_retired = fava
        .receipt(accepted.receipt_id)
        .map_err(error)?
        .ok_or_else(|| CanaryError::new("receipt disappeared after retired completion"))?;
    if after_retired.current.publication.materialization_id != MaterializationId::from_u64(2)
        || !publisher.attempts().is_empty()
    {
        return Err(CanaryError::new("retired completion mutated receipt state"));
    }
    let current_event = current.event.clone().finalize(&keys).map_err(error)?;
    current.complete(current_event)?;
    let receipt = wait_terminal(&fava, accepted.receipt_id).await?;
    let event = published_event(&receipt)?;
    let preserved = target_count(&event, "p", &bob.to_hex()) == 1
        && target_count(&event, "p", &carol.to_hex()) == 1
        && event
            .tags
            .iter()
            .any(|tag| tag.as_slice() == ["x", "unrelated", "bytes"]);
    if receipt.current.publication.materialization_id != MaterializationId::from_u64(2)
        || receipt.current.publication.materialization_source != Some(source_two.id)
        || receipt.current.publication.retired_materializations.len() != 1
        || publisher.attempts().len() != 1
        || !preserved
    {
        return Err(CanaryError::new(
            "rematerialization lifecycle facts diverged",
        ));
    }
    Ok(json!({
        "event_id": event.id.to_hex(),
        "write_id": accepted.write_id.as_u64(),
        "receipt_id": accepted.receipt_id.as_u64(),
        "first_materialization_id": 1,
        "current_materialization_id": 2,
        "source_id": source_two.id.to_hex(),
        "retired_materializations": receipt.current.publication.retired_materializations.len(),
        "publisher_attempts": publisher.attempts().len(),
        "preserved_bob_carol_unrelated": preserved,
    }))
}

async fn inverse(seed: &str) -> CanaryResult<Value> {
    let keys = deterministic_keys(&format!("{seed}-actor"))?;
    let bob = deterministic_keys(&format!("{seed}-bob"))?.public_key();
    let carol = deterministic_keys(&format!("{seed}-carol"))?.public_key();
    let bookmark_a = EventId::from_byte_array([41; 32]);
    let bookmark_b = EventId::from_byte_array([42; 32]);
    let cache = Arc::new(MemoryEventCache::default());
    let publisher = Arc::new(RecordingPublisher::default());
    let signer: Arc<dyn Signer> = Arc::new(LocalSigner::new(keys.clone()));
    let fava = assembly(
        Arc::clone(&cache),
        signer,
        selected_materializers(),
        Arc::clone(&publisher),
    )?;
    let actor = keys.public_key();
    let edits = vec![
        fava_nip02::follow(actor, bob),
        fava_nip02::follow(actor, carol),
        fava_nip02::unfollow(actor, bob),
        fava_nip02::unfollow(actor, carol),
        fava_nip02::unfollow(actor, bob),
        fava_bookmarks::bookmark_event(actor, bookmark_a),
        fava_bookmarks::bookmark_event(actor, bookmark_b),
        fava_bookmarks::unbookmark_event(actor, bookmark_a),
        fava_bookmarks::unbookmark_event(actor, bookmark_b),
        fava_bookmarks::unbookmark_event(actor, bookmark_a),
    ];
    let mut receipts = Vec::with_capacity(edits.len());
    for edit in edits {
        let accepted = fava
            .publish(explicit(edit.map_err(error)?)?)
            .map_err(error)?;
        let receipt = wait_terminal(&fava, accepted.receipt_id).await?;
        let event = published_event(&receipt)?;
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

fn n_plus_one() -> CanaryResult<Value> {
    let root = repository_root()?;
    let manifest = root.join("falsifiers/external-semantic-capability/Cargo.toml");
    for test in [
        "external_capability_composes_through_public_fava",
        "raw_future_event_kind_publishes_unchanged",
    ] {
        let status = Command::new("cargo")
            .args([
                "test",
                "--manifest-path",
                manifest.to_string_lossy().as_ref(),
                "--test",
                "public_capability",
                test,
                "--",
                "--exact",
            ])
            .current_dir(&root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() {
            return Err(CanaryError::new(format!("external proof failed: {test}")));
        }
    }
    let root_manifest = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let product_dependency = root_manifest.contains("external-semantic-capability");
    if product_dependency {
        return Err(CanaryError::new(
            "external capability entered the product graph",
        ));
    }
    Ok(json!({
        "external_manifest": "falsifiers/external-semantic-capability/Cargo.toml",
        "external_capability": true,
        "raw_future_kind": true,
        "future_kind": 50_001,
        "product_dependency": product_dependency,
    }))
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
