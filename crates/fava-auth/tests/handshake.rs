//! The handshake as an application and a relay actually see it.

mod support;

use fava_auth::{AnswerOutcome, AuthenticationDecision, SessionAuthentication};
use fava_relay::AuthenticationState;
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
    assert_eq!(rig.state(), Some(AuthenticationState::Attempted));
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

    assert_eq!(rig.state(), Some(AuthenticationState::Accepted));
    assert!(rig.authenticator().authenticated(rig.key()));
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

    let Some(AuthenticationState::AcceptedButStillRefused { message }) = rig.state() else {
        panic!("a restricted reply is authenticated-but-refused, not rejected");
    };
    assert_eq!(message.as_str(), "restricted: not a member");
    assert!(!rig.authenticator().authenticated(rig.key()));
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

    let Some(AuthenticationState::Rejected { message }) = rig.state() else {
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
    assert_eq!(rig.state(), Some(AuthenticationState::Declined));
}

#[tokio::test]
async fn no_attached_signer_fails_without_sending() {
    let rig = Rig::approving_without_signer().await;

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    assert!(auth_frames(&rig.relay()).is_empty());
    let Some(AuthenticationState::Failed { reason }) = rig.state() else {
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
    let Some(AuthenticationState::Failed { reason }) = rig.state() else {
        panic!("an oversized challenge fails rather than being truncated");
    };
    assert!(reason.as_str().contains("maximum"));
}

#[tokio::test]
async fn an_endless_re_challenge_stops_at_the_bound() {
    let rig = Rig::approving().await;

    for nonce in 0..(SessionAuthentication::MAX_ATTEMPTS + 4) {
        rig.relay()
            .push_frame(&challenge_frame(&format!("nonce-{nonce}")));
        rig.settle().await;
    }

    assert_eq!(
        auth_frames(&rig.relay()).len(),
        SessionAuthentication::MAX_ATTEMPTS as usize,
        "a relay cannot drive unbounded signing"
    );
    let Some(AuthenticationState::Failed { reason }) = rig.state() else {
        panic!("the bound is reported, not silently ignored");
    };
    assert!(reason.as_str().contains("attempt bound"));
}

#[tokio::test]
async fn a_deferred_challenge_waits_for_a_person_then_authenticates() {
    let rig = Rig::deferring().await;
    let changed = rig.authenticator().subscribe();

    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;

    assert!(
        auth_frames(&rig.relay()).is_empty(),
        "nothing is signed until the person answers"
    );
    assert_eq!(rig.state(), Some(AuthenticationState::AwaitingAnswer));
    assert!(changed.has_changed().unwrap_or(false), "the signal fired");

    let pending = rig.authenticator().pending();
    assert_eq!(pending.len(), 1, "the application can enumerate every ask");
    assert_eq!(&pending[0].session.key, rig.key());

    let outcome = rig
        .authenticator()
        .answer(pending[0].id, AuthenticationDecision::Authenticate)
        .await
        .expect("the demand awaits this answer");

    assert_eq!(outcome, AnswerOutcome::Applied);
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
    let outcome = rig
        .authenticator()
        .answer(pending[0].id, AuthenticationDecision::Decline)
        .await
        .expect("the demand awaits this answer");

    assert_eq!(outcome, AnswerOutcome::Applied);
    assert!(auth_frames(&rig.relay()).is_empty());
    assert_eq!(rig.state(), Some(AuthenticationState::Declined));
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
        .answer(pending[0].id, AuthenticationDecision::Authenticate)
        .await;

    assert!(
        matches!(outcome, Err(fava_auth::AnswerError::Unknown)),
        "an answer to a replaced connection authenticates nothing"
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
    assert!(rig.authenticator().authenticated(rig.key()));

    rig.relay().reconnect();
    rig.settle().await;

    assert!(
        !rig.authenticator().authenticated(rig.key()),
        "a replaced connection is not the one that authenticated"
    );
}

#[tokio::test]
async fn a_second_watch_on_a_live_session_keeps_its_verdict() {
    let rig = Rig::approving().await;
    rig.relay().push_frame(&challenge_frame("nonce-1"));
    rig.settle().await;
    let id = rig.last_auth_event_id();
    rig.relay().push_frame(&ok_frame(&id, true, ""));
    rig.settle().await;
    assert_eq!(rig.state(), Some(AuthenticationState::Accepted));

    // A second authenticated query on the same relay starts its own watch.
    // The connection has not been replaced, so the answer the relay already
    // accepted stands: a relay that challenges once per connection will never
    // ask again, and there would be nothing to conclude from.
    rig.authenticator()
        .watch_session(rig.key().clone())
        .await
        .expect("a second watch begins");
    rig.settle().await;

    assert_eq!(
        rig.state(),
        Some(AuthenticationState::Accepted),
        "a second watcher must not undo an accepted session"
    );
    assert!(rig.authenticator().authenticated(rig.key()));
}

#[tokio::test]
async fn a_session_reaching_a_verdict_wakes_a_watcher() {
    let rig = Rig::approving().await;
    let mut changed = rig.authenticator().subscribe();
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
    assert_eq!(rig.state(), Some(AuthenticationState::Accepted));
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
    assert_eq!(rig.state(), Some(AuthenticationState::AwaitingAnswer));
}
