//! Authentication state of one relay session, scoped to its generation.

use fava_relay::{AuthenticationState, RelaySessionKey};
use fava_transport::RelaySessionGeneration;

use crate::challenge::Challenge;

/// Current authentication state of one relay session.
///
/// A verdict belongs to the exact generation it was reached on. A reconnect
/// mints a new generation, and the session begins unauthenticated again.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionAuthentication {
    session: RelaySessionKey,
    generation: Option<RelaySessionGeneration>,
    challenge: Option<Challenge>,
    state: Option<AuthenticationState>,
    attempts: u32,
}

impl SessionAuthentication {
    /// Attempts allowed per session generation.
    ///
    /// A relay may re-challenge at any moment and some re-challenge without
    /// end regardless of whether the client answers, so signing is bounded
    /// rather than driven by the relay.
    pub const MAX_ATTEMPTS: u32 = 8;

    /// Track one authenticated-access session.
    #[must_use]
    pub const fn new(session: RelaySessionKey) -> Self {
        Self {
            session,
            generation: None,
            challenge: None,
            state: None,
            attempts: 0,
        }
    }

    /// Session this state belongs to.
    #[must_use]
    pub const fn session(&self) -> &RelaySessionKey {
        &self.session
    }

    /// Record a challenge arriving on one generation, replacing any earlier
    /// one. A challenge on a newer generation resets the attempt budget.
    pub fn challenged(&mut self, generation: RelaySessionGeneration, challenge: Challenge) {
        if self.generation != Some(generation) {
            self.reset_to(generation);
        }
        self.challenge = Some(challenge);
        self.state = Some(AuthenticationState::ChallengeReceived);
    }

    /// Record one terminal or in-flight state for a generation.
    ///
    /// A state carrying a generation the session has moved past is stale and
    /// is dropped: it describes a connection that no longer exists.
    pub fn resolved(&mut self, generation: RelaySessionGeneration, state: AuthenticationState) {
        if self.generation != Some(generation) {
            return;
        }
        if matches!(state, AuthenticationState::Attempted) {
            self.attempts = self.attempts.saturating_add(1);
        }
        self.state = Some(state);
    }

    /// Begin a replaced connection unauthenticated.
    pub fn reconnected(&mut self, generation: RelaySessionGeneration) {
        self.reset_to(generation);
    }

    /// Current state, absent until the first challenge.
    #[must_use]
    pub const fn state(&self) -> Option<&AuthenticationState> {
        self.state.as_ref()
    }

    /// Generation the current state belongs to.
    #[must_use]
    pub const fn generation(&self) -> Option<RelaySessionGeneration> {
        self.generation
    }

    /// Challenge currently outstanding for this generation.
    #[must_use]
    pub const fn challenge(&self) -> Option<&Challenge> {
        self.challenge.as_ref()
    }

    /// Whether the current generation is authenticated.
    #[must_use]
    pub const fn authenticated(&self) -> bool {
        matches!(self.state, Some(AuthenticationState::Accepted))
    }

    /// Attempts spent on the current generation.
    #[must_use]
    pub const fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Whether another attempt is allowed on this generation.
    #[must_use]
    pub const fn may_attempt(&self) -> bool {
        self.attempts < Self::MAX_ATTEMPTS
    }

    fn reset_to(&mut self, generation: RelaySessionGeneration) {
        self.generation = Some(generation);
        self.challenge = None;
        self.state = None;
        self.attempts = 0;
    }
}

#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;
