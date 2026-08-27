//! Reusable transport conformance assertions and an adversarial relay fake.
//!
//! Every assertion here is written against the neutral [`Transport`] contract,
//! so a competing implementation proves the same promises the standard
//! WebSocket implementation does.

mod fake;
mod session;
mod stream;

use std::num::NonZeroUsize;

pub use fake::{FakeRelay, FakeTransport, detached_lease};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, RelayInbound, Transport, TransportFailure,
};

/// Require that one physical session fans every inbound frame out to every
/// consumer, rather than letting consumers steal each other's frames.
///
/// The caller pushes `frame` at the relay after `arrange` returns.
///
/// # Errors
///
/// Returns a precise mismatch when either consumer misses the frame.
pub async fn require_inbound_fan_out<T, F>(
    transport: &T,
    request: OpenRelaySession,
    arrange: F,
) -> Result<(), String>
where
    T: Transport,
    F: FnOnce(),
{
    let lease = transport
        .acquire_session(request)
        .await
        .map_err(|error| format!("session did not open: {error}"))?;
    let mut first = lease.session().messages();
    let mut second = lease.session().messages();
    arrange();

    for (position, stream) in [&mut first, &mut second].into_iter().enumerate() {
        match stream.next_inbound().await {
            Ok(RelayInbound::Frame { .. }) => {}
            other => {
                return Err(format!(
                    "consumer {position} did not receive the frame, got {other:?}"
                ));
            }
        }
    }
    Ok(())
}

/// Require that acquiring a live session reuses it instead of dialing again.
///
/// # Errors
///
/// Returns a precise mismatch when the holder count or generation moved wrong.
pub async fn require_acquire_reuses_live_session<T: Transport>(
    transport: &T,
    request: OpenRelaySession,
) -> Result<(), String> {
    let key = request.key.clone();
    let first = transport
        .acquire_session(request.clone())
        .await
        .map_err(|error| format!("first acquire failed: {error}"))?;
    let second = transport
        .acquire_session(request)
        .await
        .map_err(|error| format!("second acquire failed: {error}"))?;

    if transport.holders(&key) != NonZeroUsize::new(2) {
        return Err(format!(
            "expected two holders, got {:?}",
            transport.holders(&key)
        ));
    }
    if first.session().identity() != second.session().identity() {
        return Err(format!(
            "expected one shared generation, got {:?} and {:?}",
            first.session().identity(),
            second.session().identity()
        ));
    }
    Ok(())
}

/// Require that a stalled peer converts into a bounded refusal within
/// `attempts`, never an unbounded park.
///
/// Each attempt hands over a frame at the declared per-frame bound, so a peer
/// that stops reading fills the outbound queue rather than absorbing the test.
///
/// # Errors
///
/// Returns a precise mismatch when no attempt is refused for a full queue.
pub async fn require_bounded_outbound_refusal<T: Transport>(
    transport: &T,
    request: OpenRelaySession,
    attempts: u64,
) -> Result<(), String> {
    let frame = vec![b'f'; request.bounds.max_frame_bytes.get()];
    let lease = transport
        .acquire_session(request)
        .await
        .map_err(|error| format!("session did not open: {error}"))?;
    for attempt in 0..attempts {
        let outcome = lease
            .session()
            .send(frame.clone(), HandoffCorrelation::new(attempt))
            .await;
        if let HandoffOutcome::NotHandedOff {
            reason: TransportFailure::OutboundQueueFull { .. },
            ..
        } = outcome
        {
            return Ok(());
        }
    }
    Err(format!(
        "no attempt in {attempts} produced a bounded refusal"
    ))
}

/// Require that a handoff completion names the exact generation that made it.
///
/// # Errors
///
/// Returns a precise mismatch when identity or correlation is not carried back.
pub async fn require_attributed_handoff<T: Transport>(
    transport: &T,
    request: OpenRelaySession,
) -> Result<(), String> {
    let lease = transport
        .acquire_session(request)
        .await
        .map_err(|error| format!("session did not open: {error}"))?;
    let expected = lease.session().identity();
    let outcome = lease
        .session()
        .send(
            b"[\"REQ\",\"conformance\",{}]".to_vec(),
            HandoffCorrelation::new(41),
        )
        .await;

    if outcome.identity() != &expected {
        return Err(format!(
            "completion named {:?}, expected {expected:?}",
            outcome.identity()
        ));
    }
    if outcome.correlation() != HandoffCorrelation::new(41) {
        return Err(format!(
            "completion carried correlation {:?}",
            outcome.correlation()
        ));
    }
    Ok(())
}
