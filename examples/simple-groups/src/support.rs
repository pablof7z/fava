//! Explicit Fava assembly and bounded receipt evidence for this domain app.

use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, Receipt, Write, all_acknowledged};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer_local::LocalSigner;
use fava_simple_groups::saved_group_list_materializer;
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

pub(super) const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);

pub(super) fn assemble(alice: &Keys, bob: &Keys) -> Result<Fava, fava::BuildError> {
    Fava::builder()
        .event_cache_ephemeral()
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::new()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .signer(Arc::new(LocalSigner::new(alice.clone())))
        .signer(Arc::new(LocalSigner::new(bob.clone())))
        .materializers([saved_group_list_materializer()])
        .build()
}

pub(super) fn settle_acknowledged(write: &Write) -> Result<Receipt, String> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            tokio::time::timeout(OPERATION_TIMEOUT, write.settled(all_acknowledged()))
                .await
                .map_err(|_| {
                    format!(
                        "write {} did not reach acknowledgement within {OPERATION_TIMEOUT:?}",
                        write.write_id().as_u64()
                    )
                })?
                .map_err(|error| error.to_string())
        })
    })
}
