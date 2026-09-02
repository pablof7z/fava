//! NIP-42 challenge and authorization lifecycles for exact relay access.
//!
//! Authentication belongs to a relay connection, not to a message. One
//! lifecycle exists per connection whose live [`fava_relay::Authentication`]
//! is on its way to [`fava_relay::Authentication::Authenticated`], identified
//! by the exact relay and transport generation it was established on. Queries
//! and publications that join an authenticated session inherit it; neither
//! performs a handshake.
//!
//! The application supplies one [`AuthenticationPolicy`] for the engine. It
//! decides synchronously and performs no effects: authenticate, decline, or
//! defer to a person. A deferred challenge signs nothing and parks the work
//! that needs it until the answer arrives.

mod authenticator;
mod challenge;
mod demand;
mod event;
mod policy;

pub use authenticator::{AnswerError, Authenticator, MAX_ATTEMPTS, WatchError};
pub use challenge::{Challenge, ChallengeError};
pub use demand::AuthenticationDemand;
pub use event::auth_event;
pub use policy::{AuthenticationDecision, AuthenticationPolicy};
