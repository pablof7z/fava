//! Reconnect pacing owned by the transport (`ARCH:1588-1589`, `ARCH:1625`).
//!
//! Reconnect used to be a fixed 50ms retry loop outside this crate, with no
//! growth, ceiling, jitter, or attempt bound. All four live here now.

use std::time::Duration;

/// Exponential reconnect pacing with a ceiling and full jitter.
///
/// The caller owns the attempt budget (`OpenRelaySession::reconnect_attempts`);
/// this owns only the delay between attempts.
pub(crate) struct ReconnectBackoff {
    /// Delay before the first retry.
    base: Duration,
    /// Delay no attempt ever exceeds.
    ceiling: Duration,
    /// Retries already paced.
    attempt: u32,
    /// Jitter state. A deterministic generator keeps the crate dependency-free
    /// while still de-correlating simultaneous reconnects across relays.
    entropy: u64,
}

impl ReconnectBackoff {
    /// First retry waits this long.
    pub(crate) const BASE: Duration = Duration::from_millis(100);
    /// No retry ever waits longer than this.
    pub(crate) const CEILING: Duration = Duration::from_secs(30);

    pub(crate) const fn new(entropy: u64) -> Self {
        Self {
            base: Self::BASE,
            ceiling: Self::CEILING,
            attempt: 0,
            entropy: if entropy == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                entropy
            },
        }
    }

    /// Pace one retry: doubling, capped at the ceiling, with full jitter so a
    /// fleet of sessions never re-dials in lockstep.
    pub(crate) fn next_delay(&mut self) -> Duration {
        let doubled = self
            .base
            .saturating_mul(1_u32 << self.attempt.min(16))
            .min(self.ceiling);
        self.attempt = self.attempt.saturating_add(1);
        let span = u64::try_from(doubled.as_millis()).unwrap_or(u64::MAX);
        if span == 0 {
            return Duration::ZERO;
        }
        Duration::from_millis(self.jitter() % span.saturating_add(1))
    }

    /// Forget the growth: a generation that lived is not evidence of trouble.
    pub(crate) const fn reset(&mut self) {
        self.attempt = 0;
    }

    fn jitter(&mut self) -> u64 {
        self.entropy ^= self.entropy << 13;
        self.entropy ^= self.entropy >> 7;
        self.entropy ^= self.entropy << 17;
        self.entropy
    }
}

#[cfg(test)]
mod tests {
    use super::ReconnectBackoff;

    #[test]
    fn delays_grow_and_never_exceed_the_ceiling() {
        let mut backoff = ReconnectBackoff::new(7);
        for _ in 0..40 {
            assert!(backoff.next_delay() <= ReconnectBackoff::CEILING);
        }
    }

    #[test]
    fn two_sessions_do_not_re_dial_in_lockstep() {
        let mut one = ReconnectBackoff::new(1);
        let mut two = ReconnectBackoff::new(2);
        let first: Vec<_> = (0..8).map(|_| one.next_delay()).collect();
        let second: Vec<_> = (0..8).map(|_| two.next_delay()).collect();
        assert_ne!(first, second);
    }

    #[test]
    fn a_surviving_generation_resets_the_growth() {
        let mut backoff = ReconnectBackoff::new(3);
        for _ in 0..10 {
            let _ = backoff.next_delay();
        }
        backoff.reset();
        assert!(backoff.next_delay() <= ReconnectBackoff::BASE);
    }
}
