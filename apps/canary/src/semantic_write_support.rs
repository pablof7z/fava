//! Private support for deterministic semantic-write canaries.

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{
    Event, EventValue, Fava, Observation, PublicKey, Receipt, ReceiptId, RelayUrl,
    ReplaceableEventEdit, ReplaceableEventMaterializer, UnsignedEvent, WriteIntent, WriteRouting,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_state::{RelayAccess, RelayEvidence, RelaySessionKey, Timestamp};
use fava_transport::{RelaySession, Transport, TransportError};
use nostr::event::UnsignedEvent as NostrUnsignedEvent;
use nostr::key::Keys;
use secp256k1::Secp256k1;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::artifacts::RunArtifacts;
use crate::semantic_write_store::{CompletionAck, CompletionStore};
use crate::{CanaryError, CanaryResult, SmokeOptions, command_output, repository_root};

const SIGN_REQUEST_CAPACITY: usize = 2;

#[derive(Default)]
pub(super) struct RecordingPublisher {
    attempts: Mutex<Vec<PublishAttempt>>,
}

impl RecordingPublisher {
    pub(super) fn attempts(&self) -> Vec<PublishAttempt> {
        self.attempts.lock().expect("attempt lock poisoned").clone()
    }
}

impl Publisher for RecordingPublisher {
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(async move {
            self.attempts
                .lock()
                .expect("attempt lock poisoned")
                .push(attempt);
            PublishOutcome::Acknowledged {
                message: "deterministic acknowledgement".to_owned(),
            }
        })
    }
}

struct NoopTransport;

impl Transport for NoopTransport {
    fn open_session(
        &self,
        _session: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>
    {
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                "deterministic canary transport".to_owned(),
            ))
        })
    }
}

type SignResponse = oneshot::Sender<Result<Event, SignerError>>;

pub(super) struct PendingSign {
    pub(super) event: UnsignedEvent,
    response: SignResponse,
}

impl PendingSign {
    pub(super) fn complete(self, event: Event) -> CanaryResult<()> {
        self.response
            .send(Ok(event))
            .map_err(|_| CanaryError::new("signing generation was no longer awaiting completion"))
    }
}

pub(super) struct GateSigner {
    public_key: PublicKey,
    requests: mpsc::Sender<PendingSign>,
}

impl GateSigner {
    pub(super) fn new(public_key: PublicKey) -> (Self, mpsc::Receiver<PendingSign>) {
        let (requests, receiver) = mpsc::channel(SIGN_REQUEST_CAPACITY);
        (
            Self {
                public_key,
                requests,
            },
            receiver,
        )
    }
}

impl Signer for GateSigner {
    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn sign_event(
        &self,
        event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>> {
        Box::pin(async move {
            let (response, completion) = oneshot::channel();
            self.requests
                .try_send(PendingSign { event, response })
                .map_err(|error| {
                    SignerError::Unavailable(format!("signing request refused: {error}"))
                })?;
            completion
                .await
                .map_err(|_| SignerError::Unavailable("completion channel closed".to_owned()))?
        })
    }
}

pub(super) struct DeterministicSigner {
    keys: Keys,
}

impl DeterministicSigner {
    pub(super) const fn new(keys: Keys) -> Self {
        Self { keys }
    }
}

impl Signer for DeterministicSigner {
    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    fn sign_event(
        &self,
        event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>> {
        Box::pin(async move { deterministic_finalize(event, &self.keys) })
    }
}

pub(super) fn assembly<P>(
    cache: Arc<MemoryEventCache>,
    signer: Arc<dyn Signer>,
    materializers: Vec<Arc<dyn ReplaceableEventMaterializer>>,
    publisher: Arc<P>,
) -> CanaryResult<(Fava, broadcast::Receiver<CompletionAck>)>
where
    P: Publisher + 'static,
{
    let (store, completions) = CompletionStore::new();
    let mut builder = Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signers([signer])
        .publisher(publisher)
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()));
    for materializer in materializers {
        builder = builder.materializers([materializer]);
    }
    Ok((builder.build().map_err(error)?, completions))
}

pub(super) fn explicit(edit: ReplaceableEventEdit, author: PublicKey) -> CanaryResult<WriteIntent> {
    WriteIntent::edit_as(
        edit,
        author,
        WriteRouting::Explicit(BTreeSet::from([relay_url()])),
    )
    .map_err(error)
}

pub(super) fn explicit_event(event: UnsignedEvent) -> CanaryResult<WriteIntent> {
    WriteIntent::event(event, WriteRouting::Explicit(BTreeSet::from([relay_url()]))).map_err(error)
}

fn relay_url() -> RelayUrl {
    RelayUrl::parse("wss://m7-semantic.example").expect("fixed relay URL is valid")
}

pub(super) fn relay_evidence() -> RelayEvidence {
    RelayEvidence::one(
        RelaySessionKey::new(relay_url(), RelayAccess::public()),
        Timestamp::from(1),
    )
}

pub(super) async fn next_sign(
    requests: &mut mpsc::Receiver<PendingSign>,
) -> CanaryResult<PendingSign> {
    tokio::time::timeout(Duration::from_secs(5), requests.recv())
        .await
        .map_err(|_| CanaryError::new("timed out awaiting materialization signing"))?
        .ok_or_else(|| CanaryError::new("signing request channel closed"))
}

pub(super) async fn wait_completion(
    completions: &mut broadcast::Receiver<CompletionAck>,
    materialization_id: u64,
) -> CanaryResult<CompletionAck> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match completions.recv().await {
                Ok(completion)
                    if completion.materialization_id
                        == fava::MaterializationId::from_u64(materialization_id) =>
                {
                    return Ok(completion);
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    return Err(CanaryError::new(format!(
                        "completion acknowledgement lagged by {skipped}"
                    )));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(CanaryError::new(
                        "completion acknowledgement channel closed",
                    ));
                }
            }
        }
    })
    .await
    .map_err(|_| CanaryError::new("timed out awaiting completion acknowledgement"))?
}

pub(super) async fn wait_terminal(fava: &Fava, receipt_id: ReceiptId) -> CanaryResult<Receipt> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let receipt = fava
                .receipt(receipt_id)
                .map_err(error)?
                .ok_or_else(|| CanaryError::new("accepted write receipt disappeared"))?;
            if receipt.is_terminal() {
                return Ok(receipt);
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .map_err(|_| CanaryError::new("timed out awaiting terminal receipt"))?
}

pub(super) async fn wait_query_event(
    observation: &mut Observation,
    event_id: fava_write::EventId,
) -> CanaryResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if observation
                .current()
                .events
                .iter()
                .any(|record| record.event.id() == Some(event_id))
            {
                return Ok(());
            }
            observation
                .changed()
                .await
                .map_err(|_| CanaryError::new("public observation closed"))?;
        }
    })
    .await
    .map_err(|_| CanaryError::new("timed out awaiting public query update"))?
}

pub(super) fn published_event(receipt: &Receipt) -> CanaryResult<Event> {
    match &receipt.current.event {
        EventValue::Signed(event) => Ok(event.clone()),
        EventValue::Unsigned(_) => {
            Err(CanaryError::new("terminal receipt retained unsigned event"))
        }
    }
}

pub(super) fn deterministic_finalize(
    mut event: NostrUnsignedEvent,
    keys: &Keys,
) -> Result<Event, SignerError> {
    if event.pubkey != keys.public_key() {
        return Err(SignerError::InvalidOutput(
            "unsigned event author does not match signer".to_owned(),
        ));
    }
    let id = event.id.unwrap_or_else(|| event.compute_id());
    event.id = Some(id);
    let signature = keys.sign_schnorr_with_aux_rand(&Secp256k1::new(), id.as_bytes(), &[0; 32]);
    event
        .add_signature(signature)
        .map_err(|error| SignerError::InvalidOutput(error.to_string()))
}

pub(super) fn attempt_evidence(
    accepted: &fava_write_store::AcceptedWrite,
    receipt: &Receipt,
    attempt: &PublishAttempt,
) -> CanaryResult<Value> {
    let exact_session = receipt.desired_destinations.len() == 1
        && receipt.desired_destinations.contains(&attempt.session)
        && receipt.attempts.get(&attempt.session) == Some(&attempt.number);
    if attempt.write_id != accepted.write_id
        || attempt.receipt_id != accepted.receipt_id
        || receipt.write_id != accepted.write_id
        || receipt.receipt_id != accepted.receipt_id
        || attempt.materialization_id != receipt.current.publication.materialization_id
        || attempt.event.id != receipt.current.id()
        || attempt.number != 1
        || !exact_session
    {
        return Err(CanaryError::new(
            "publisher attempt correlation diverged from exact receipt state",
        ));
    }
    Ok(json!({
        "write_id": attempt.write_id.as_u64(),
        "receipt_id": attempt.receipt_id.as_u64(),
        "materialization_id": attempt.materialization_id.as_u64(),
        "event_id": attempt.event.id.to_hex(),
        "receipt_event_id": receipt.current.id().to_hex(),
        "created_at": attempt.event.created_at.as_secs(),
        "receipt_created_at": receipt.current.event.created_at().as_secs(),
        "session": attempt.session.relay.to_string(),
        "attempt": attempt.number,
    }))
}

pub(super) fn target_count(event: &Event, marker: &str, value: &str) -> usize {
    event
        .tags
        .iter()
        .filter(|tag| {
            let fields = tag.as_slice();
            fields.first().is_some_and(|field| field == marker)
                && fields.get(1).is_some_and(|field| field == value)
        })
        .count()
}

pub(super) fn seed_hash(seed: &str) -> String {
    hex::encode(Sha256::digest(seed.as_bytes()))
}

pub(super) fn finish(
    mut artifacts: RunArtifacts,
    id: &str,
    options: &SmokeOptions,
    details: &Value,
) -> CanaryResult<std::path::PathBuf> {
    artifacts.record("scenario_passed", details.clone())?;
    artifacts.write_json("semantic.json", details)?;
    artifacts.write_report(&format!("{id} completed through public Fava\n"))?;
    let root = repository_root()?;
    let revision = command_output(&root, "git", &["rev-parse", "HEAD"])?;
    let dirty = !command_output(&root, "git", &["status", "--porcelain"])?.is_empty();
    let artifact_hashes = artifacts.artifact_hashes()?;
    artifacts.write_json(
        "manifest.json",
        &json!({
            "run_id": artifacts.run_id()?, "scenario": id,
            "scenario_seed_sha256": seed_hash(&options.seed),
            "selected_profile": "memory-public-fava", "fava_revision": revision,
            "canary_revision": revision, "dirty": dirty,
            "relay_implementation": Value::Null, "artifact_sha256": artifact_hashes,
        }),
    )?;
    Ok(artifacts.root().to_path_buf())
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use fava::{EventBuilder, Kind, Timestamp};
    use fava_signer::{Signer, SignerError};
    use nostr::key::Keys;
    use tokio::sync::watch;

    use super::GateSigner;

    #[tokio::test(flavor = "current_thread")]
    async fn signer_request_queue_refuses_work_beyond_its_exact_bound() {
        let keys = Keys::generate();
        let (signer, requests) = GateSigner::new(keys.public_key());
        let signer = Arc::new(signer);
        let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
            .created_at(Timestamp::from(1))
            .build()
            .unwrap();
        let first_signer = Arc::clone(&signer);
        let first_event = event.clone();
        let first = tokio::spawn(async move {
            first_signer
                .sign_event(first_event, watch::channel(false).1)
                .await
        });
        let second_signer = Arc::clone(&signer);
        let second_event = event.clone();
        let second = tokio::spawn(async move {
            second_signer
                .sign_event(second_event, watch::channel(false).1)
                .await
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while requests.len() != 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("two requests fill the exact queue bound");
        let refusal = tokio::time::timeout(
            Duration::from_millis(25),
            signer.sign_event(event, watch::channel(false).1),
        )
        .await
        .expect("overflow refuses without waiting");
        assert!(matches!(refusal, Err(SignerError::Unavailable(_))));
        first.abort();
        second.abort();
    }
}
