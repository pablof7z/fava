//! Exact receipt-generation activation for router sessions and route effects.

use fava_routing::{RoutePlan, RouteRequest, RouterSession};
use fava_write::{EventValue, Receipt, WriteRouting};
use fava_write_store::destination_evidence_capacity;
use tokio::sync::watch;

use super::Publication;
use super::materialization::SemanticState;

impl Publication {
    pub(super) async fn open_generation(
        &self,
        mut receipt: Receipt,
        semantic: &mut Option<SemanticState>,
        cancel: &mut watch::Receiver<bool>,
    ) -> Option<(Receipt, Option<Box<dyn RouterSession>>)> {
        for _ in 0..=destination_evidence_capacity() {
            if receipt.is_terminal() {
                return None;
            }
            if let Some(state) = semantic {
                receipt = self.initialize_semantic(receipt, state, cancel).await?;
            }
            let WriteRouting::Automatic = receipt.routing else {
                let current = self.read_receipt(receipt.receipt_id, cancel).await?;
                if current == receipt {
                    return Some((current, None));
                }
                receipt = current;
                continue;
            };

            let request = RouteRequest::Write(receipt.current.event.clone());
            let opened = fava_routing::open(self.routers.as_slice(), &request);
            let current = self.read_receipt(receipt.receipt_id, cancel).await?;
            if current != receipt {
                if let Ok(mut routes) = opened {
                    routes.close();
                }
                receipt = current;
                continue;
            }

            match opened {
                Ok(mut routes) => {
                    let revision = receipt.route_revision.saturating_add(1);
                    let plan = RoutePlan::from_contribution(revision, &routes.current())
                        .unwrap_or_else(|error| {
                            RoutePlan::shortfall(revision, &request, error.to_string())
                        });
                    match self.store.apply_route(
                        receipt.write_id,
                        receipt.receipt_id,
                        receipt.current.publication.materialization_id,
                        receipt.current.id(),
                        &plan,
                    ) {
                        Ok(committed) => return Some((committed, Some(routes))),
                        Err(_) => routes.close(),
                    }
                }
                Err(error) => {
                    let revision = receipt.route_revision.saturating_add(1);
                    let plan = RoutePlan::shortfall(revision, &request, error.to_string());
                    if let Ok(committed) = self.store.apply_route(
                        receipt.write_id,
                        receipt.receipt_id,
                        receipt.current.publication.materialization_id,
                        receipt.current.id(),
                        &plan,
                    ) {
                        return Some((committed, None));
                    }
                }
            }
            receipt = self.read_receipt(receipt.receipt_id, cancel).await?;
        }
        self.record_activation_exhaustion(&receipt);
        None
    }

    /// Mark signing retryable after the route-activation retry bound ran out
    /// without a route settling.
    pub(super) fn record_activation_exhaustion(&self, receipt: &Receipt) {
        if !matches!(receipt.current.event, EventValue::Unsigned(_)) {
            return;
        }
        let _ = self.store.record_signer_retryable(
            receipt.write_id,
            receipt.receipt_id,
            receipt.current.publication.materialization_id,
            receipt.current.id(),
            format!(
                "generation activation retry bound {} exhausted for write {} receipt {} materialization {} event {}; retry is permitted after a receipt or provider change",
                destination_evidence_capacity() + 1,
                receipt.write_id.as_u64(),
                receipt.receipt_id.as_u64(),
                receipt.current.publication.materialization_id.as_u64(),
                receipt.current.id(),
            ),
        );
    }
}
