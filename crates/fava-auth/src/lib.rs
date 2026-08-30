//! NIP-42 challenge and authorization lifecycles for exact relay access.
//!
//! Authentication belongs to a relay connection, not to a message. One
//! lifecycle exists per [`fava_relay::RelaySessionKey`] whose access is
//! [`fava_relay::RelayAccess::Authenticated`], scoped to the transport
//! generation it was established on. Queries and publications that join an
//! authenticated session inherit it; neither performs a handshake.
//!
//! The application supplies one [`AuthenticationPolicy`] for the engine. It
//! decides synchronously and performs no effects: authenticate, decline, or
//! defer to a person. A deferred challenge signs nothing and parks the work
//! that needs it until the answer arrives.

mod challenge;
mod demand;
mod event;
mod policy;
mod state;

pub use challenge::{Challenge, ChallengeError};
pub use demand::{AuthenticationDemand, AuthenticationDemandId, PendingAuthentication};
pub use event::auth_event;
pub use policy::{AuthenticationDecision, AuthenticationPolicy};
pub use state::SessionAuthentication;
