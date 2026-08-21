use std::path::{Path, PathBuf};
use std::sync::Arc;

use fava::Fava;
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_routing::Router;
use fava_signer::Signer;
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_redb::RedbWriteStore;

use crate::artifacts::RunArtifacts;
use crate::relay::{ProcessFact, RelayProcess, RelaySupervisor};
use crate::{CanaryError, CanaryResult, SmokeOptions, WireProxy, reserve_port};

pub(crate) struct LabRelay {
    pub(crate) process: RelayProcess,
    pub(crate) proxy: WireProxy,
    pub(crate) url: String,
    pub(crate) log: PathBuf,
}

impl LabRelay {
    pub(crate) async fn stop(self) -> CanaryResult<ProcessFact> {
        let fact = self.process.graceful_stop().await?;
        self.proxy.shutdown().await?;
        Ok(fact)
    }
}

pub(crate) async fn start_relays(
    artifacts: &mut RunArtifacts,
    options: &SmokeOptions,
    count: usize,
) -> CanaryResult<(String, Vec<LabRelay>, Vec<ProcessFact>)> {
    let mut version = None;
    let mut relays = Vec::new();
    let mut facts = Vec::new();
    for index in 0..count {
        let directory = artifacts
            .root()
            .join(format!("relays/nostr-rs-relay-{index}"));
        let supervisor =
            RelaySupervisor::prepare(&options.relay_binary, &directory, reserve_port().await?)?;
        version.get_or_insert(supervisor.version().await?);
        let process = supervisor.spawn(1).await?;
        facts.push(process.fact("ready"));
        artifacts.record("relay_ready", process.fact("ready"))?;
        let log = artifacts.root().join(format!("wire/proxy-{index}.jsonl"));
        let proxy = WireProxy::start(supervisor.address(), &log).await?;
        relays.push(LabRelay {
            url: proxy.url(),
            process,
            proxy,
            log,
        });
    }
    Ok((
        version.ok_or_else(|| CanaryError::new("M6 scenario started no relay"))?,
        relays,
        facts,
    ))
}

pub(crate) fn query_fava(cache: Arc<MemoryEventCache>) -> CanaryResult<Fava> {
    Fava::builder()
        .event_cache(cache)
        .write_store(Arc::new(
            fava_write_store_memory::MemoryWriteStore::default(),
        ))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
        .build()
        .map_err(error)
}

pub(crate) fn publication_fava(
    cache: Arc<MemoryEventCache>,
    database: &Path,
    routers: impl IntoIterator<Item = Arc<dyn Router>>,
    signer: Option<Arc<dyn Signer>>,
) -> CanaryResult<Fava> {
    let mut builder = Fava::builder()
        .event_cache(cache)
        .write_store(Arc::new(RedbWriteStore::open(database).map_err(error)?))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .routers(routers);
    if let Some(signer) = signer {
        builder = builder.signers([signer]);
    }
    builder.build().map_err(error)
}

pub(crate) fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
