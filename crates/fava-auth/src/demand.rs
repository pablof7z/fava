//! One relay demand that a session authenticate.

use fava_transport::RelaySessionIdentity;

use crate::challenge::Challenge;

/// One relay demand that this session authenticate.
///
/// The identity names the relay and the exact transport generation the
/// challenge arrived on. A verdict never applies to a later generation. The
/// account to authenticate as is not here: the policy names it as part of
/// deciding to authenticate at all.
///
/// A demand a policy defers to a person is not a separate, longer-lived
/// value: the connection named by `session` carries it for as long as it is
/// outstanding (`fava_relay::Progress::Requested`), and answering it names
/// that same connection. Nothing here mints an identity of its own.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticationDemand {
    /// Relay and transport generation the challenge arrived on.
    pub session: RelaySessionIdentity,
    /// Current challenge for that generation.
    pub challenge: Challenge,
}
