//! Same-URL observations with different access own independent live work.

mod support;

use fava_query::{AuthenticationState, Query, RelaySourceState};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_transport::Transport;
use fava_wire::RelayMessage;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use support::{assemble, push, relay, requests, settle, wait_until};

#[tokio::test(flavor = "current_thread")]
#[allow(
    clippy::too_many_lines,
    reason = "one causal sequence covers EVENT, EOSE, AUTH, CLOSED, reconnect, and withdrawal isolation"
)]
async fn exact_access_keys_isolate_event_eose_and_close() -> Result<(), Box<dyn std::error::Error>>
{
    let assembly = assemble();
    let url = relay("shared-access");
    let alice = Keys::generate().public_key();
    let public_key = RelaySessionKey {
        relay: url.clone(),
        access: RelayAccess::Public,
    };
    let private_key = RelaySessionKey {
        relay: url.clone(),
        access: RelayAccess::Authenticated(alice),
    };
    let public = assembly.observer.open(
        Query::events()
            .only_from_relays([url.clone()])?
            .with_relay_access(RelayAccess::Public),
    )?;
    let private = assembly.observer.open(
        Query::events()
            .only_from_relays([url])?
            .with_relay_access(RelayAccess::Authenticated(alice)),
    )?;

    wait_until(|| assembly.transport.relay(&public_key).is_some()).await;
    wait_until(|| assembly.transport.relay(&private_key).is_some()).await;
    let public_peer = assembly.transport.relay(&public_key).unwrap();
    let private_peer = assembly.transport.relay(&private_key).unwrap();
    wait_until(|| requests(Some(public_peer.clone())).len() == 1).await;
    wait_until(|| requests(Some(private_peer.clone())).len() == 1).await;
    let public_wire = requests(Some(public_peer.clone()))[0].0.clone();
    let mut private_wire = requests(Some(private_peer.clone()))[0].0.clone();
    assert_ne!(public_wire, private_wire);

    let public_event = EventBuilder::new(Kind::TextNote, "public").finalize(&Keys::generate())?;
    push(
        &public_peer,
        &RelayMessage::event(public_wire.clone(), public_event.clone()),
    );
    wait_until(|| {
        public
            .current()
            .events
            .iter()
            .any(|record| record.id() == public_event.id)
    })
    .await;
    assert!(private.current().events.is_empty());

    push(&public_peer, &RelayMessage::eose(public_wire));
    wait_until(|| {
        public
            .current()
            .evidence
            .relay(&public_key)
            .is_some_and(fava_query::RelayQueryEvidence::stored_events_complete)
    })
    .await;
    assert!(
        !private
            .current()
            .evidence
            .relay(&private_key)
            .unwrap()
            .stored_events_complete()
    );

    push(&public_peer, &RelayMessage::auth("public challenge"));
    wait_until(|| {
        matches!(
            public
                .current()
                .evidence
                .relay(&public_key)
                .map(|item| &item.state),
            Some(RelaySourceState::AuthenticationRequired {
                state: AuthenticationState::ChallengeReceived,
                ..
            })
        )
    })
    .await;
    assert!(matches!(
        private
            .current()
            .evidence
            .relay(&private_key)
            .map(|item| &item.state),
        Some(RelaySourceState::Open { .. })
    ));

    push(
        &public_peer,
        &RelayMessage::closed(
            requests(Some(public_peer.clone()))[0].0.clone(),
            "public refused",
        ),
    );
    wait_until(|| {
        matches!(
            public
                .current()
                .evidence
                .relay(&public_key)
                .map(|item| &item.state),
            Some(RelaySourceState::Refused { .. })
        )
    })
    .await;
    assert!(!matches!(
        private
            .current()
            .evidence
            .relay(&private_key)
            .map(|item| &item.state),
        Some(RelaySourceState::Refused { .. } | RelaySourceState::AuthenticationRequired { .. })
    ));

    let private_generation = private
        .current()
        .evidence
        .relay(&private_key)
        .expect("private evidence")
        .generation
        .expect("live relay work has an exact generation");
    let stale_wire = private_wire.clone();
    private_peer.reconnect();
    wait_until(|| requests(Some(private_peer.clone())).len() == 2).await;
    private_wire = requests(Some(private_peer.clone()))[1].0.clone();
    wait_until(|| {
        private
            .current()
            .evidence
            .relay(&private_key)
            .is_some_and(|item| {
                item.generation
                    .is_some_and(|current| current > private_generation)
            })
    })
    .await;

    let stale_event =
        EventBuilder::new(Kind::TextNote, "stale generation").finalize(&Keys::generate())?;
    push(
        &private_peer,
        &RelayMessage::event(stale_wire, stale_event.clone()),
    );
    settle().await;
    assert!(
        private
            .current()
            .events
            .iter()
            .all(|record| record.id() != stale_event.id),
        "an event naming the superseded generation's exact wire request is inert"
    );
    assert!(matches!(
        public
            .current()
            .evidence
            .relay(&public_key)
            .map(|item| &item.state),
        Some(RelaySourceState::Refused { .. })
    ));

    public.close();
    settle().await;
    assert!(assembly.transport.holders(&public_key).is_none());
    assert!(assembly.transport.holders(&private_key).is_some());
    let private_event = EventBuilder::new(Kind::TextNote, "private").finalize(&Keys::generate())?;
    push(
        &private_peer,
        &RelayMessage::event(private_wire, private_event.clone()),
    );
    wait_until(|| {
        private
            .current()
            .events
            .iter()
            .any(|record| record.id() == private_event.id)
    })
    .await;
    private.close();
    Ok(())
}
