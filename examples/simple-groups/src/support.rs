//! Explicit Fava assembly and bounded acknowledgement evidence for this app.

use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, PublishError, Receipt, Write, all_acknowledged, all_terminal};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_memory::MemoryWriteStore;

pub(crate) const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);

pub(crate) fn assemble() -> Result<Fava, fava::BuildError> {
    Fava::builder()
        .event_cache_ephemeral()
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::new()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
}

/// Receipt evidence observed while waiting for acknowledgement.
pub(crate) enum AcknowledgementSettlement {
    /// Every desired relay acknowledged the current materialization.
    Acknowledged(Receipt),
    /// Routing became terminal before acknowledgement sufficiency.
    Terminal(Receipt),
    /// The app timeout elapsed while the receipt remained observable.
    TimedOut(Receipt),
}

/// Receipt evidence observed while waiting for all relay outcomes to become terminal.
pub(crate) enum TerminalSettlement {
    /// Every current destination has a terminal fact.
    Terminal(Receipt),
    /// The app timeout elapsed while the receipt remained observable.
    TimedOut(Receipt),
}

pub(crate) fn settle_acknowledged(write: &Write) -> Result<AcknowledgementSettlement, String> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            match tokio::time::timeout(OPERATION_TIMEOUT, write.settled(all_acknowledged())).await {
                Ok(Ok(receipt)) => Ok(AcknowledgementSettlement::Acknowledged(receipt)),
                Ok(Err(PublishError::NotReached { receipt })) => {
                    Ok(AcknowledgementSettlement::Terminal(receipt))
                }
                Ok(Err(error)) => Err(error.to_string()),
                Err(_) => write
                    .receipt()
                    .map(AcknowledgementSettlement::TimedOut)
                    .map_err(|error| error.to_string()),
            }
        })
    })
}

pub(crate) fn settle_terminal(write: &Write) -> Result<TerminalSettlement, String> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            match tokio::time::timeout(OPERATION_TIMEOUT, write.settled(all_terminal())).await {
                Ok(Ok(receipt)) => Ok(TerminalSettlement::Terminal(receipt)),
                Ok(Err(PublishError::NotReached { receipt })) => {
                    Ok(TerminalSettlement::Terminal(receipt))
                }
                Ok(Err(error)) => Err(error.to_string()),
                Err(_) => write
                    .receipt()
                    .map(TerminalSettlement::TimedOut)
                    .map_err(|error| error.to_string()),
            }
        })
    })
}
