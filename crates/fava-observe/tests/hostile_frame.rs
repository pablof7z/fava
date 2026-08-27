//! Gate 2: malformed inbound frames are rejected without crashing other sessions.
//!
//! Proves: pushing bad UTF-8, unparseable JSON, and an EVENT attributed to a
//! subscription this session never opened are all silently discarded. No panic,
//! no session teardown; a subsequent valid EOSE on the same relay is handled
//! correctly.

mod support;

use fava_query::{Query, RelaySourceState};
use fava_wire::RelayMessage;
use nostr::event::Kind;
use nostr::key::Keys;
use support::{assemble, push, relay, relay_evidence, requests, settle, wait_until};

/// Bad frames on a relay session do not affect the observation or crash it.
#[tokio::test(flavor = "current_thread")]
async fn malformed_frames_are_silently_discarded_without_crash() {
    let r = relay("hostile");
    let author = Keys::generate().public_key();

    let assembly = assemble();
    let obs = assembly
        .observer
        .open(
            Query::events()
                .kinds([Kind::TextNote])
                .expect("kind bounded")
                .authors([author])
                .expect("author bounded")
                .only_from_relays([r.clone()])
                .expect("explicit relay valid"),
        )
        .expect("observation opens");

    // Wait for the REQ to be sent.
    wait_until(|| requests(assembly.peer(&r)).len() == 1).await;
    let peer = assembly.established(&r);

    // Push malformed frames: raw bytes, invalid JSON, truncated JSON.
    peer.push_frame(b"\xff\xfe not utf8".to_vec());
    peer.push_frame(b"not json at all".to_vec());
    peer.push_frame(b"[\"EVE".to_vec());

    // Push an EOSE with the correct subscription id to prove the session lives.
    let sub_id = requests(assembly.peer(&r))[0].0.clone();
    push(&peer, &RelayMessage::eose(sub_id));

    // Allow all frames to be processed.
    settle().await;
    settle().await;

    // The EOSE must have been processed (StoredEventsComplete or Open state).
    let evidence = relay_evidence(&obs, &r);
    assert!(
        matches!(
            evidence.state,
            RelaySourceState::StoredEventsComplete { .. } | RelaySourceState::Open { .. }
        ),
        "session still live after hostile frames; state: {:?}",
        evidence.state
    );

    // No event was admitted.
    assert!(
        obs.current().events.is_empty(),
        "malformed frames must not admit events"
    );

    obs.close();
}

/// An EVENT for a subscription id this session never opened is discarded;
/// the next valid EOSE on the real subscription still works.
#[tokio::test(flavor = "current_thread")]
async fn off_subscription_event_is_rejected_and_session_continues() {
    let r = relay("hostile2");
    let author = Keys::generate().public_key();

    let assembly = assemble();
    let obs = assembly
        .observer
        .open(
            Query::events()
                .kinds([Kind::TextNote])
                .expect("kind bounded")
                .authors([author])
                .expect("author bounded")
                .only_from_relays([r.clone()])
                .expect("explicit relay valid"),
        )
        .expect("observation opens");

    wait_until(|| requests(assembly.peer(&r)).len() == 1).await;
    let peer = assembly.established(&r);
    let sub_id = requests(assembly.peer(&r))[0].0.clone();

    // Push raw bytes that look like an EVENT for a nonexistent subscription.
    // This is valid JSON but names a ghost subscription — ingest rejects it.
    peer.push_frame(
        br#"["EVENT","ghost-subscription-id",{"id":"0000000000000000000000000000000000000000000000000000000000000000","pubkey":"0000000000000000000000000000000000000000000000000000000000000000","created_at":0,"kind":1,"tags":[],"content":"","sig":"0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"}]"#
        .to_vec(),
    );

    // The real EOSE should still be processed.
    push(&peer, &RelayMessage::eose(sub_id));

    settle().await;
    settle().await;

    let evidence = relay_evidence(&obs, &r);
    assert!(
        matches!(
            evidence.state,
            RelaySourceState::StoredEventsComplete { .. } | RelaySourceState::Open { .. }
        ),
        "session alive after off-subscription frame; state: {:?}",
        evidence.state
    );

    obs.close();
}
