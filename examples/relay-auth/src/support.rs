//! Explicit real-relay Fava assembly, and the one runtime-switchable NIP-42
//! policy this application hands it.

use std::sync::{Arc, Mutex, PoisonError};

use fava::Fava;
use fava_auth::{AuthenticationDecision, AuthenticationDemand, AuthenticationPolicy};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_memory::MemoryWriteStore;

/// A person's current answer to every future NIP-42 challenge, changeable
/// while the engine is running.
///
/// `fava_auth::AuthenticationPolicy` is selected once, at assembly time, and
/// decides synchronously with no memory of its own — the same shape as
/// `fava_delivery::DeliveryPolicy`. An application that wants a person to flip
/// the answer for challenges still to come owns that switch itself and hands
/// the trait a read of it; this is that switch.
pub(crate) struct SwitchablePolicy {
    decision: Mutex<AuthenticationDecision>,
}

impl SwitchablePolicy {
    pub(crate) fn new(initial: AuthenticationDecision) -> Self {
        Self {
            decision: Mutex::new(initial),
        }
    }

    pub(crate) fn set(&self, decision: AuthenticationDecision) {
        *self.decision.lock().unwrap_or_else(PoisonError::into_inner) = decision;
    }

    fn get(&self) -> AuthenticationDecision {
        *self.decision.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl AuthenticationPolicy for SwitchablePolicy {
    fn decide(&self, _demand: &AuthenticationDemand) -> AuthenticationDecision {
        self.get()
    }
}

pub(crate) fn assemble(policy: Arc<SwitchablePolicy>) -> Result<Fava, fava::BuildError> {
    Fava::builder()
        .event_cache_ephemeral()
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::new()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .authentication_policy(policy)
        .build()
}
