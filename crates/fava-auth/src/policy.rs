//! The application's decision seam.

use fava_write::PublicKey;

use crate::demand::AuthenticationDemand;

/// Application decision for one demand.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AuthenticationDecision {
    /// Answer the challenge as this exact account. Deciding to authenticate
    /// and deciding as whom are one decision: nothing else names the account
    /// any more, since access stopped being part of a connection's identity.
    Authenticate {
        /// The account to answer the challenge as.
        as_of: PublicKey,
    },
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
