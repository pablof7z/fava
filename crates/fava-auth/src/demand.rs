//! One relay demand that a session authenticate, and its deferred form.

use std::num::NonZeroU64;

use fava_transport::RelaySessionIdentity;

use crate::challenge::Challenge;

/// One relay demand that this session authenticate.
///
/// The identity names the relay and the exact transport generation the
/// challenge arrived on. A verdict never applies to a later generation. The
/// account to authenticate as is not here: the policy names it as part of
/// deciding to authenticate at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticationDemand {
    /// Relay and transport generation the challenge arrived on.
    pub session: RelaySessionIdentity,
    /// Current challenge for that generation.
    pub challenge: Challenge,
}

/// Stable identity of one demand awaiting a person's answer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthenticationDemandId(NonZeroU64);

impl AuthenticationDemandId {
    /// Build an identity from a non-zero counter value.
    #[must_use]
    pub const fn from_nonzero(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Exact identity value.
    #[must_use]
    pub const fn get(self) -> NonZeroU64 {
        self.0
    }
}

/// One demand a policy deferred to a person, still awaiting an answer.
///
/// The challenge itself is not exposed: an application decides on the relay
/// and the account, and echoing an opaque relay nonce into a user interface
/// tells nobody anything.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingAuthentication {
    /// Stable identity for answering this exact demand.
    pub id: AuthenticationDemandId,
    /// Relay and generation the demand belongs to.
    pub session: RelaySessionIdentity,
}
