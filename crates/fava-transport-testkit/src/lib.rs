//! Reusable transport conformance assertions.

use fava_transport::{HandoffOutcome, RelaySession, TransportError};

/// Require successful handoff for one provider-arranged frame.
///
/// # Errors
///
/// Returns a precise mismatch if the provider reports refusal or ambiguity.
pub async fn require_handoff_success(
    session: &dyn RelaySession,
    frame: String,
) -> Result<(), String> {
    match session.send(frame).await {
        HandoffOutcome::HandedOff => Ok(()),
        other => Err(format!("expected successful handoff, got {other:?}")),
    }
}

/// Require definite pre-handoff refusal for one provider-arranged frame.
///
/// # Errors
///
/// Returns a precise mismatch if the provider reports success or ambiguity.
pub async fn require_handoff_refusal(
    session: &dyn RelaySession,
    frame: String,
) -> Result<(), String> {
    match session.send(frame).await {
        HandoffOutcome::NotHandedOff { .. } => Ok(()),
        other => Err(format!("expected handoff refusal, got {other:?}")),
    }
}

/// Require the next read to report a disconnected session.
///
/// # Errors
///
/// Returns a precise mismatch if the provider returns a frame or another fact.
pub async fn require_disconnect(session: &dyn RelaySession) -> Result<(), String> {
    match session.next_message().await {
        Err(TransportError::Disconnected(_)) => Ok(()),
        other => Err(format!("expected disconnect, got {other:?}")),
    }
}

/// Require idempotent close and definite refusal after close.
///
/// # Errors
///
/// Returns a precise mismatch for a close failure or post-close handoff.
pub async fn require_idempotent_close(session: &dyn RelaySession) -> Result<(), String> {
    session.close().await.map_err(|error| error.to_string())?;
    session.close().await.map_err(|error| error.to_string())?;
    require_handoff_refusal(session, "after-close".to_owned()).await
}
