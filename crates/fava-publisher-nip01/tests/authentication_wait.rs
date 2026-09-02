//! A write accepted under an account waits on its connection's authentication
//! rather than failing when a relay demands it.
//!
//! Nothing here plays the authentication owner's role: these tests write
//! directly into the connection through `RelaySessionExt`, exactly as
//! `fava-auth` does, and never touch the publisher or the write it is
//! carrying. That the publisher resumes anyway is the proof that it is
//! reading the connection, not being told anything by whoever decided.

use std::num::NonZeroU64;
use std::time::Duration;

use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_publisher_nip01::Nip01Publisher;
use fava_relay::{Authority, BoundedText, Progress};
use fava_transport::RelaySessionExt;
use fava_transport_testkit::{FakeRelay, FakeTransport};
use fava_write::{ReceiptId, RevisionId, WriteId};
use nostr::event::FinalizeEvent;
use nostr::key::{Keys, PublicKey};
use nostr::types::RelayUrl;

/// Bound every polling wait in this file: a regression in the wait-on-
/// connection behavior must fail these tests, never hang the process.
const DEADLINE: Duration = Duration::from_secs(5);

fn key() -> RelayUrl {
    RelayUrl::parse("ws://127.0.0.1:1/").expect("relay URL")
}

fn attempt(event: nostr::event::Event, authority: Authority) -> PublishAttempt {
    let one = NonZeroU64::MIN;
    PublishAttempt {
        write_id: WriteId::from_nonzero(one),
        receipt_id: ReceiptId::from_nonzero(one),
        revision_id: RevisionId::FIRST,
        number: 1,
        session: key(),
        authority,
        event,
        timeout: Duration::from_secs(5),
    }
}

fn event() -> nostr::event::Event {
    let keys = Keys::generate();
    nostr::event::EventBuilder::new(nostr::event::Kind::TextNote, "gm")
        .finalize(&keys)
        .expect("event signs")
}

/// Wait until `transport` has registered a connection reachable for
/// `relay`/`authority`, then return it. Bounded: a connection that never
/// appears fails the test instead of hanging it.
async fn wait_for_session(
    transport: &FakeTransport,
    relay: &RelayUrl,
    authority: &Authority,
) -> std::sync::Arc<dyn fava_transport::RelaySession> {
    tokio::time::timeout(DEADLINE, async {
        loop {
            if let Some(session) = transport.session(relay, authority) {
                return session;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("a connection for this relay and authority never appeared")
}

/// Wait until `relay` has recorded at least `count` delivered frames.
/// Bounded: a resend that never happens fails the test instead of hanging it.
async fn wait_for_frames(relay: &FakeRelay, count: usize) {
    tokio::time::timeout(DEADLINE, async {
        loop {
            if relay.delivered_frames().len() >= count {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the relay never received {count} frame(s)"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_write_parked_on_auth_required_resumes_once_the_connection_is_accepted() {
    let transport = FakeTransport::new();
    let publisher = Nip01Publisher;
    let account: PublicKey = Keys::generate().public_key();
    let authority = Authority::As(account);
    let event = event();
    let id = event.id;

    let driver = {
        let transport = transport.clone();
        tokio::spawn(async move {
            let session = wait_for_session(&transport, &key(), &authority).await;
            let relay = transport
                .relay(&key(), &authority)
                .expect("relay is registered");

            // First round: the relay demands authentication instead of an
            // ordinary verdict.
            wait_for_frames(&relay, 1).await;
            let refusal =
                serde_json::json!(["OK", id.to_hex(), false, "auth-required: who are you"])
                    .to_string();
            relay.push_frame(refusal.as_bytes());

            // Nobody tells the publisher anything; the connection is simply
            // brought to the state the write is waiting for.
            RelaySessionExt::record_accepted(&session, account);

            // Second round: the publisher must have sent the same event
            // again on its own, now that the connection can carry it.
            wait_for_frames(&relay, 2).await;
            let acceptance = serde_json::json!(["OK", id.to_hex(), true, "stored"]).to_string();
            relay.push_frame(acceptance.as_bytes());
            relay.delivered_frames().len()
        })
    };

    let outcome = tokio::time::timeout(
        DEADLINE,
        publisher.publish(attempt(event, authority), &transport),
    )
    .await
    .expect("the publish call must settle within the deadline");
    let frames = driver.await.expect("driver task completes");

    assert_eq!(
        outcome,
        PublishOutcome::Acknowledged {
            message: "stored".to_owned(),
        },
        "the write must resume and succeed once the connection is accepted, without \
         ever being told that it happened"
    );
    assert_eq!(
        frames, 2,
        "exactly one resend must follow the demand, not a busy loop of them"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_write_parked_on_auth_required_fails_when_the_relay_refuses_the_answer() {
    let transport = FakeTransport::new();
    let publisher = Nip01Publisher;
    let account: PublicKey = Keys::generate().public_key();
    let authority = Authority::As(account);
    let event = event();
    let id = event.id;

    let driver = {
        let transport = transport.clone();
        tokio::spawn(async move {
            let session = wait_for_session(&transport, &key(), &authority).await;
            let relay = transport
                .relay(&key(), &authority)
                .expect("relay is registered");
            wait_for_frames(&relay, 1).await;
            let refusal = serde_json::json!(["OK", id.to_hex(), false, "auth-required: prove it"])
                .to_string();
            relay.push_frame(refusal.as_bytes());

            RelaySessionExt::record_progress(
                &session,
                Progress::Refused {
                    reason: BoundedText::new("not on this list"),
                },
            );
        })
    };

    let outcome = tokio::time::timeout(
        DEADLINE,
        publisher.publish(attempt(event, authority), &transport),
    )
    .await
    .expect("the publish call must settle within the deadline");
    driver.await.expect("driver task completes");

    let PublishOutcome::AuthenticationRequired { message } = outcome else {
        panic!("expected a denial naming the relay's refusal, got {outcome:?}");
    };
    assert!(
        message.contains("authentication was required and did not happen"),
        "message was: {message}"
    );
    assert!(
        message.contains("not on this list"),
        "the relay's own words must survive: {message}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_write_awaiting_a_person_neither_resumes_nor_fails_until_they_decide() {
    let transport = FakeTransport::new();
    let account: PublicKey = Keys::generate().public_key();
    let authority = Authority::As(account);
    let event = event();
    let id = event.id;

    let (phase_one_reached, phase_one_signal) = tokio::sync::oneshot::channel::<()>();
    let (proceed, proceed_signal) = tokio::sync::oneshot::channel::<()>();

    let driver = {
        let transport = transport.clone();
        tokio::spawn(async move {
            let session = wait_for_session(&transport, &key(), &authority).await;
            let relay = transport
                .relay(&key(), &authority)
                .expect("relay is registered");
            wait_for_frames(&relay, 1).await;
            let refusal =
                serde_json::json!(["OK", id.to_hex(), false, "auth-required: sign in"]).to_string();
            relay.push_frame(refusal.as_bytes());
            // The connection is left exactly where a still-undecided
            // challenge leaves it: nobody has approved or declined.
            RelaySessionExt::record_progress(
                &session,
                Progress::Requested {
                    challenge: "prove it".to_owned(),
                },
            );
            let _ = phase_one_reached.send(());

            // Only once the test has confirmed the write is still waiting
            // does anyone decide anything.
            proceed_signal.await.expect("the test signals to proceed");
            RelaySessionExt::record_accepted(&session, account);
            wait_for_frames(&relay, 2).await;
            let acceptance = serde_json::json!(["OK", id.to_hex(), true, "stored"]).to_string();
            relay.push_frame(acceptance.as_bytes());
        })
    };

    let mut publish = tokio::spawn({
        let transport = transport.clone();
        async move {
            let publisher = Nip01Publisher;
            publisher
                .publish(attempt(event, authority), &transport)
                .await
        }
    });

    tokio::time::timeout(DEADLINE, phase_one_signal)
        .await
        .expect("the driver must reach the undecided challenge within the deadline")
        .expect("the driver reaches the undecided challenge");

    // A person has not decided, so the write must still be waiting: neither
    // an acknowledgement nor a denial has been produced.
    tokio::select! {
        result = &mut publish => panic!(
            "a write awaiting a person must stay open, not settle while nobody has \
             decided: {result:?}"
        ),
        () = tokio::time::sleep(Duration::from_millis(100)) => {}
    }

    // Only now does the person answer, and only then does the write move.
    let _ = proceed.send(());
    let outcome = tokio::time::timeout(DEADLINE, publish)
        .await
        .expect("the publish task must complete within the deadline")
        .expect("publish task completes");
    tokio::time::timeout(DEADLINE, driver)
        .await
        .expect("the driver task must complete within the deadline")
        .expect("driver task completes");

    assert_eq!(
        outcome,
        PublishOutcome::Acknowledged {
            message: "stored".to_owned(),
        },
        "the write must resume once the person decides"
    );
}
