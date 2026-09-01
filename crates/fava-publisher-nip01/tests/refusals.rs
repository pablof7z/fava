//! What this publisher makes of a relay's `OK` refusal.
//!
//! NIP-01 makes an `OK` refusal's prefix machine-readable, and NIP-42 defines
//! `auth-required:` among them. A publisher that flattened every refusal to
//! `Rejected` would leave the authentication owner nothing to act on, so the
//! distinction is proved here rather than assumed.

use std::num::NonZeroU64;
use std::time::Duration;

use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_publisher_nip01::Nip01Publisher;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_transport_testkit::FakeTransport;
use fava_write::{ReceiptId, RevisionId, WriteId};
use nostr::event::FinalizeEvent;
use nostr::types::RelayUrl;

fn key() -> RelaySessionKey {
    RelaySessionKey {
        relay: RelayUrl::parse("ws://127.0.0.1:1/").expect("relay URL"),
        access: RelayAccess::Public,
    }
}

fn attempt(event: nostr::event::Event) -> PublishAttempt {
    let one = NonZeroU64::MIN;
    PublishAttempt {
        write_id: WriteId::from_nonzero(one),
        receipt_id: ReceiptId::from_nonzero(one),
        revision_id: RevisionId::FIRST,
        number: 1,
        session: key(),
        event,
        timeout: Duration::from_secs(5),
    }
}

fn event() -> nostr::event::Event {
    let keys = nostr::key::Keys::generate();
    nostr::event::EventBuilder::new(nostr::event::Kind::TextNote, "gm")
        .finalize(&keys)
        .expect("event signs")
}

/// Publish `event`, answer its `EVENT` frame with `OK false "<message>"`, and
/// return what the publisher concluded.
async fn refused_with(message: &str) -> PublishOutcome {
    let transport = FakeTransport::new();
    let publisher = Nip01Publisher;
    let event = event();
    let id = event.id;
    let relay = {
        // The relay registers on the first dial, so answer from a task that
        // waits for the publisher's own frame to arrive.
        let transport = transport.clone();
        let message = message.to_owned();
        tokio::spawn(async move {
            loop {
                if let Some(relay) = transport.relay(&key())
                    && !relay.delivered_frames().is_empty()
                {
                    let frame = serde_json::json!(["OK", id.to_hex(), false, message]).to_string();
                    relay.push_frame(frame.as_bytes());
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
    };
    let outcome = publisher.publish(attempt(event), &transport).await;
    relay.await.expect("the relay answers");
    outcome
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_auth_required_refusal_is_a_demand_not_a_rejection() {
    let message = "auth-required: we only serve authenticated users";
    assert_eq!(
        refused_with(message).await,
        PublishOutcome::AuthenticationRequired {
            message: message.to_owned(),
        },
        "the relay's demand must reach the authentication owner in its own words"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_other_refusal_stays_an_ordinary_rejection() {
    let message = "restricted: this relay does not accept notes from you";
    assert_eq!(
        refused_with(message).await,
        PublishOutcome::Rejected {
            message: message.to_owned(),
        },
        "only the prefix NIP-42 defines means authentication would have helped"
    );
}
