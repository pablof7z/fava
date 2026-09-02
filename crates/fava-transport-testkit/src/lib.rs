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
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, SubscriptionItem, Transport,
    TransportFailure,
};
use fava_wire::SubscriptionId;
use nostr::event::Kind;
use nostr::filter::Filter;

/// Require that a relay message reaches the handle that owns its wire key, and
/// no other.
///
/// The caller pushes `frame` at the relay after `arrange` returns; `frame` must
/// be an `EVENT` naming the subscription the returned handle holds.
///
/// # Errors
///
/// Returns a precise mismatch when the owning handle misses the message, or
/// when a second subscription receives it.
pub async fn require_routed_delivery<T, F>(
    transport: &T,
    request: OpenRelaySession,
    arrange: F,
) -> Result<(), String>
where
    T: Transport,
    F: FnOnce(&SubscriptionId),
{
    let lease = transport
        .acquire_session(request)
        .await
        .map_err(|error| format!("session did not open: {error}"))?;
    let session = std::sync::Arc::clone(lease.session());
    let mut owner = fava_transport::RelaySessionExt::subscribe(&session, vec![Filter::new()])
        .await
        .map_err(|refused| format!("subscription did not open: {refused:?}"))?;
    let mut bystander = fava_transport::RelaySessionExt::subscribe(
        &session,
        vec![Filter::new().kind(Kind::from_u16(1))],
    )
    .await
    .map_err(|refused| format!("second subscription did not open: {refused:?}"))?;
    arrange(owner.id());

    match owner.next().await {
        SubscriptionItem::Event(_) => {}
        other => return Err(format!("the owning subscription received {other:?}")),
    }
    // The bystander must still be waiting: nothing addressed to another
    // subscription may reach it.
    match tokio::time::timeout(std::time::Duration::from_millis(50), bystander.next()).await {
        Err(_) => Ok(()),
        Ok(item) => Err(format!("an unrelated subscription received {item:?}")),
    }
}

/// Require that a message no live handle claims reaches no handle at all.
///
/// The caller pushes an unroutable frame after `arrange` returns.
///
/// # Errors
///
/// Returns a precise mismatch when a live handle receives it.
pub async fn require_unclaimed_reaches_no_handle<T, F>(
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
    let session = std::sync::Arc::clone(lease.session());
    let mut subscription =
        fava_transport::RelaySessionExt::subscribe(&session, vec![Filter::new()])
            .await
            .map_err(|refused| format!("subscription did not open: {refused:?}"))?;
    arrange();

    match tokio::time::timeout(std::time::Duration::from_millis(50), subscription.next()).await {
        Err(_) => Ok(()),
        Ok(item) => Err(format!(
            "an unclaimed message reached a subscription: {item:?}"
        )),
    }
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
    let relay = request.relay.clone();
    let authority = request.authority;
    let first = transport
        .acquire_session(request.clone())
        .await
        .map_err(|error| format!("first acquire failed: {error}"))?;
    let second = transport
        .acquire_session(request)
        .await
        .map_err(|error| format!("second acquire failed: {error}"))?;

    if transport.holders(&relay, &authority) != NonZeroUsize::new(2) {
        return Err(format!(
            "expected two holders, got {:?}",
            transport.holders(&relay, &authority)
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
            .hand_off(frame.clone(), HandoffCorrelation::new(attempt))
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
        .hand_off(
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
