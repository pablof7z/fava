//! The application's decision seam.

use crate::demand::AuthenticationDemand;

/// Application decision for one demand.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AuthenticationDecision {
    /// Answer the challenge as the account named by the session key.
    Authenticate,
    /// Do not authenticate to this relay under this account.
    Decline,
    /// A person owns this answer. Nothing is signed or sent, and work needing
    /// the session parks until the answer arrives.
    Defer,
}

/// Replaceable application policy deciding whether to authenticate.
///
/// Decides synchronously and performs no effects, like
/// `fava_delivery::DeliveryPolicy`. A policy that needs a person answers
/// [`AuthenticationDecision::Defer`] and returns immediately, rather than
/// holding the decision open inside the owner.
pub trait AuthenticationPolicy: Send + Sync {
    /// Decide without performing effects or retaining a ledger.
    fn decide(&self, demand: &AuthenticationDemand) -> AuthenticationDecision;
}

impl<F> AuthenticationPolicy for F
where
    F: Fn(&AuthenticationDemand) -> AuthenticationDecision + Send + Sync,
{
    fn decide(&self, demand: &AuthenticationDemand) -> AuthenticationDecision {
        self(demand)
    }
}
