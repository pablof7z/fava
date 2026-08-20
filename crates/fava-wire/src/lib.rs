//! Exact NIP-01 client and relay wire messages.

pub use nostr::message::{ClientMessage, RelayMessage, SubscriptionId};

/// Encode one exact NIP-01 client message as a text frame.
///
/// # Errors
///
/// Returns a JSON error if the protocol value cannot be serialized.
pub fn encode_client(message: &ClientMessage<'_>) -> Result<String, serde_json::Error> {
    serde_json::to_string(message)
}

/// Decode one exact NIP-01 relay text frame into owned protocol values.
///
/// # Errors
///
/// Returns a JSON error if the frame is not a recognized relay message.
pub fn decode_relay(frame: &str) -> Result<RelayMessage<'static>, serde_json::Error> {
    serde_json::from_str(frame)
}
