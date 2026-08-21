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
use fava_write_store_memory::MemoryWriteStore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, oneshot, watch};

use crate::artifacts::RunArtifacts;
use crate::{CanaryError, CanaryResult, SmokeOptions, command_output, repository_root};

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
    requests: mpsc::UnboundedSender<PendingSign>,
}

impl GateSigner {
    pub(super) fn new(public_key: PublicKey) -> (Self, mpsc::UnboundedReceiver<PendingSign>) {
        let (requests, receiver) = mpsc::unbounded_channel();
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
                .send(PendingSign { event, response })
                .map_err(|_| SignerError::Unavailable("request channel closed".to_owned()))?;
            completion
                .await
                .map_err(|_| SignerError::Unavailable("completion channel closed".to_owned()))?
        })
    }
}

pub(super) fn assembly(
    cache: Arc<MemoryEventCache>,
    signer: Arc<dyn Signer>,
    materializers: Vec<Arc<dyn ReplaceableEventMaterializer>>,
    publisher: Arc<RecordingPublisher>,
) -> CanaryResult<Fava> {
    let mut builder = Fava::builder()
        .event_cache(cache)
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signers([signer])
        .publisher(publisher)
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()));
    for materializer in materializers {
        builder = builder.materializers([materializer]);
    }
    builder.build().map_err(error)
}

pub(super) fn explicit(edit: ReplaceableEventEdit) -> CanaryResult<WriteIntent> {
    WriteIntent::edit(edit, WriteRouting::Explicit(BTreeSet::from([relay_url()]))).map_err(error)
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
    requests: &mut mpsc::UnboundedReceiver<PendingSign>,
) -> CanaryResult<PendingSign> {
    tokio::time::timeout(Duration::from_secs(5), requests.recv())
        .await
        .map_err(|_| CanaryError::new("timed out awaiting materialization signing"))?
        .ok_or_else(|| CanaryError::new("signing request channel closed"))
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
