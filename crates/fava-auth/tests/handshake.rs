//! The handshake as an application and a relay actually see it.

mod support;

use fava_auth::{AnswerError, AuthenticationDecision, MAX_ATTEMPTS};
use fava_relay::Progress;
use nostr::event::FinalizeEvent;
use support::{Rig, auth_frames, challenge_frame, ok_frame};

#[tokio::test]
async fn a_challenge_is_answered_with_no_publication_in_flight() {
    let rig = Rig::approving().await;

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    assert_eq!(
        auth_frames(&rig.relay()).len(),
        1,
        "the owner's own lease sees an unsolicited challenge"
    );
    assert!(matches!(rig.progress(), Progress::Answering { .. }));
}

#[tokio::test]
async fn one_approved_challenge_sends_exactly_one_auth_frame() {
    let rig = Rig::approving().await;

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;
    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    assert_eq!(
        auth_frames(&rig.relay()).len(),
        1,
        "a relay repeating its challenge has not asked a second question"
    );
}

#[tokio::test]
async fn a_new_challenge_is_answered_even_on_the_same_connection() {
    let rig = Rig::approving().await;

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;
    rig.relay().push_frame(&challenge_frame("nonce-2"));
    rig.settle().await;

    assert_eq!(
        auth_frames(&rig.relay()).len(),
        2,
        "a different challenge is a different question"
    );
}

#[tokio::test]
async fn an_accepted_response_authenticates_the_session() {
    let rig = Rig::approving().await;
    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    let id = rig.last_auth_event_id();
    rig.relay().push_frame(&ok_frame(&id, true, ""));
    rig.settle().await;

    assert!(rig.established().is_some());
    assert!(rig.established().is_some());
}

#[tokio::test]
async fn a_restricted_reply_is_not_a_rejection() {
    let rig = Rig::approving().await;
    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    let id = rig.last_auth_event_id();
    rig.relay()
        .push_frame(&ok_frame(&id, false, "restricted: not a member"));
    rig.settle().await;

    let Progress::Refused { reason: message } = rig.progress() else {
        panic!("a restricted reply is authenticated-but-refused, not rejected");
    };
    assert_eq!(message.as_str(), "restricted: not a member");
    assert!(rig.established().is_none());
}

#[tokio::test]
async fn a_rejected_response_keeps_the_relays_own_text() {
    let rig = Rig::approving().await;
    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    let id = rig.last_auth_event_id();
    rig.relay()
        .push_frame(&ok_frame(&id, false, "error: bad signature"));
    rig.settle().await;

    let Progress::Refused { reason: message } = rig.progress() else {
        panic!("a non-restricted refusal is a rejection");
    };
    assert_eq!(message.as_str(), "error: bad signature");
}

#[tokio::test]
async fn a_declining_policy_signs_nothing() {
    let rig = Rig::declining().await;

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    assert!(
        auth_frames(&rig.relay()).is_empty(),
        "a declined challenge never reaches the wire"
    );
    assert!(matches!(rig.progress(), Progress::Declined));
}

#[tokio::test]
async fn no_attached_signer_fails_without_sending() {
    let rig = Rig::approving_without_signer().await;

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    assert!(auth_frames(&rig.relay()).is_empty());
    let Progress::Unanswerable { reason } = rig.progress() else {
        panic!("a missing signer fails this attempt");
    };
    assert!(reason.as_str().contains("no signer"));
}

#[tokio::test]
async fn an_oversized_challenge_is_refused_without_signing() {
    let rig = Rig::approving().await;

    rig.relay().push_frame(&challenge_frame(
        &"x".repeat(fava_auth::Challenge::MAX_BYTES + 1),
    ));
    rig.settle().await;

    assert!(auth_frames(&rig.relay()).is_empty());
    let Progress::Unanswerable { reason } = rig.progress() else {
        panic!("an oversized challenge fails rather than being truncated");
    };
    assert!(reason.as_str().contains("maximum"));
}

#[tokio::test]
async fn an_endless_re_challenge_stops_at_the_bound() {
    let rig = Rig::approving().await;

    for nonce in 0..(MAX_ATTEMPTS + 4) {
        rig.relay()
            .push_frame(&challenge_frame(&format!("nonce-{nonce}")));
        rig.settle().await;
    }

    assert_eq!(
        auth_frames(&rig.relay()).len(),
        MAX_ATTEMPTS as usize,
        "a relay cannot drive unbounded signing"
    );
    let Progress::Unanswerable { reason } = rig.progress() else {
        panic!("the bound is reported, not silently ignored");
    };
    assert!(reason.as_str().contains("attempt bound"));
}

#[tokio::test]
async fn a_deferred_challenge_waits_for_a_person_then_authenticates() {
    let rig = Rig::deferring().await;
    let changed = fava_transport::RelaySessionExt::connection(&rig.session());

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    assert!(
        auth_frames(&rig.relay()).is_empty(),
        "nothing is signed until the person answers"
    );
    assert!(matches!(rig.progress(), Progress::Requested { .. }));
    assert!(changed.has_changed().unwrap_or(false), "the signal fired");

    let pending = rig.authenticator().pending();
    assert_eq!(pending.len(), 1, "the application can enumerate every ask");
    assert_eq!(&pending[0].relay, rig.relay_url());

    rig.authenticator()
        .answer(
            &pending[0],
            AuthenticationDecision::Authenticate {
                as_of: rig.account(),
            },
        )
        .await
        .expect("the demand awaits this answer");

    rig.settle().await;
    assert_eq!(auth_frames(&rig.relay()).len(), 1);
    assert!(
        rig.authenticator().pending().is_empty(),
        "an answered demand stops asking"
    );
}

#[tokio::test]
async fn a_person_may_refuse_a_deferred_challenge() {
    let rig = Rig::deferring().await;
    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    let pending = rig.authenticator().pending();
    rig.authenticator()
        .answer(&pending[0], AuthenticationDecision::Decline)
        .await
        .expect("the demand awaits this answer");

    assert!(auth_frames(&rig.relay()).is_empty());
    assert!(matches!(rig.progress(), Progress::Declined));
}

#[tokio::test]
async fn a_reconnect_drops_an_outstanding_demand_and_a_stale_answer_does_nothing() {
    let rig = Rig::deferring().await;
    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    let pending = rig.authenticator().pending();
    assert_eq!(pending.len(), 1);

    rig.relay().reconnect();
    rig.settle().await;

    assert!(
        rig.authenticator().pending().is_empty(),
        "a demand dies with the connection that raised it"
    );

    let outcome = rig
        .authenticator()
        .answer(
            &pending[0],
            AuthenticationDecision::Authenticate {
                as_of: rig.account(),
            },
        )
        .await;

    // The connection this identity named is gone; no other record of the
    // demand survives it, so a stale answer is indistinguishable from one
    // that was never asked at all.
    assert!(
        matches!(outcome, Err(AnswerError::Unknown)),
        "an answer to a replaced connection applies to nothing, got {outcome:?}"
    );
    assert!(auth_frames(&rig.relay()).is_empty());
}

#[tokio::test]
async fn a_reconnected_session_begins_unauthenticated() {
    let rig = Rig::approving().await;
    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;
    let id = rig.last_auth_event_id();
    rig.relay().push_frame(&ok_frame(&id, true, ""));
    rig.settle().await;
    assert!(rig.established().is_some());

    rig.relay().reconnect();
    rig.settle().await;

    assert!(
        rig.established().is_none(),
        "a replaced connection is not the one that authenticated"
    );
}

#[tokio::test]
async fn a_session_reaching_a_verdict_wakes_a_watcher() {
    let rig = Rig::approving().await;
    let mut changed = fava_transport::RelaySessionExt::connection(&rig.session());
    changed.mark_unchanged();

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;
    assert!(
        changed.has_changed().unwrap_or(false),
        "answering the challenge is a change worth waking for"
    );
    changed.mark_unchanged();

    let id = rig.last_auth_event_id();
    rig.relay().push_frame(&ok_frame(&id, true, ""));
    rig.settle().await;

    // Without this the only way to learn a session finished authenticating is
    // to keep asking it.
    assert!(
        changed.has_changed().unwrap_or(false),
        "the relay accepting the answer is a change worth waking for"
    );
    assert!(rig.established().is_some());
}

#[tokio::test]
async fn a_relay_that_re_challenges_asks_a_person_once() {
    let rig = Rig::deferring().await;

    for nonce in ["nonce-1", "nonce-2", "nonce-3"] {
        rig.relay().push_frame(&challenge_frame(nonce));
        rig.settle().await;
    }

    // One connection is one conversation. A relay repeating itself has not
    // asked three questions, and a person cannot answer a challenge that has
    // already been superseded.
    let pending = rig.authenticator().pending();
    assert_eq!(
        pending.len(),
        1,
        "a re-challenge replaces the outstanding ask, got {pending:?}"
    );
    assert!(matches!(rig.progress(), Progress::Requested { .. }));
}

#[tokio::test]
async fn an_answer_signed_for_a_connection_that_is_gone_reaches_no_relay() {
    let rig = Rig::approving().await;
    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;
    assert_eq!(auth_frames(&rig.relay()).len(), 1);

    // The connection is replaced, which resets it to having been asked
    // nothing. An answer signed for the connection that is gone must not be
    // put on the one that replaced it.
    rig.relay().reconnect();
    rig.settle().await;

    let event = nostr::event::EventBuilder::new(nostr::event::Kind::Authentication, "")
        .finalize(&nostr::key::Keys::generate())
        .expect("event signs");
    let refused = fava_transport::RelaySessionExt::answer(&rig.session(), event).await;

    assert!(
        refused.is_err(),
        "a connection that was never asked accepts no answer"
    );
    assert_eq!(
        auth_frames(&rig.relay()).len(),
        1,
        "and nothing further reaches the relay"
    );
}

#[tokio::test]
async fn a_relay_repeating_itself_is_not_a_new_question() {
    let rig = Rig::deferring().await;
    let mut changed = fava_transport::RelaySessionExt::connection(&rig.session());
    changed.mark_unchanged();

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;
    assert!(
        changed.has_changed().unwrap_or(false),
        "the first ask woke it"
    );
    changed.mark_unchanged();

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    // The relay said the same thing again. Nothing about this connection
    // changed, so nobody is woken and nobody is asked twice.
    assert!(
        !changed.has_changed().unwrap_or(false),
        "a repeated challenge is not a change"
    );
    assert_eq!(rig.authenticator().pending().len(), 1);
}

#[tokio::test]
async fn a_new_nonce_is_answered_against_the_one_the_relay_last_sent() {
    let rig = Rig::approving().await;

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.relay().push_frame(&challenge_frame("nonce-2"));
    rig.settle().await;

    // Whatever order the answers were signed in, the connection ends up
    // answering the question the relay last asked, not an earlier one.
    let frames = auth_frames(&rig.relay());
    let answered: Vec<_> = frames
        .iter()
        .filter_map(|frame| {
            frame[1]["tags"]
                .as_array()?
                .iter()
                .find(|tag| tag[0] == "challenge")?[1]
                .as_str()
                .map(str::to_owned)
        })
        .collect();
    assert!(
        answered.contains(&"nonce-2".to_owned()),
        "the newest question is the one that gets answered, got {answered:?}"
    );
}
