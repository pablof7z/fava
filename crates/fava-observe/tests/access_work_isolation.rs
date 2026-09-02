//! Public and authenticated demand at the same URL keep independent evidence,
//! and once a connection is committed to an account it owns independent live
//! work too.

mod support;

use fava_query::{Query, RelaySourceState};
use fava_relay::Authority;
use fava_transport::RelaySessionExt;
use fava_wire::RelayMessage;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use nostr::types::RelayUrl;
use support::{Assembly, assemble, push, relay, requests, settle, wait_until};

struct Setup {
    assembly: Assembly,
    url: RelayUrl,
    public: fava_observe::Observation,
    private: fava_observe::Observation,
    public_peer: fava_transport_testkit::FakeRelay,
    public_wire: fava_wire::SubscriptionId,
}

/// Open a public and an authenticated-as-alice observation at the same relay
/// URL, and return once both connections have committed and each has one
/// live wire subscription.
///
/// The private observation opens and authenticates first, so its connection
/// is committed to alice before the public observation ever asks: a
/// connection already authenticated as alice can never become anonymous
/// again, so the two are guaranteed distinct connections from here on.
async fn setup() -> Result<Setup, Box<dyn std::error::Error>> {
    let assembly = assemble();
    let url = relay("shared-access");
    let alice = Keys::generate().public_key();

    let private = assembly.observer.open(
        Query::events()
            .only_from_relays([url.clone()])?
            .with_relay_access(Authority::As(alice)),
    )?;
    wait_until(|| {
        assembly
            .transport
            .relay(&url, &Authority::As(alice))
            .is_some()
    })
    .await;
    let private_peer = assembly
        .transport
        .relay(&url, &Authority::As(alice))
        .unwrap();
    let private_session = assembly
        .transport
        .session(&url, &Authority::As(alice))
        .expect("the watch acquired this session");
    RelaySessionExt::record_accepted(&private_session, alice);

    let public = assembly.observer.open(
        Query::events()
            .only_from_relays([url.clone()])?
            .with_relay_access(Authority::Unauthenticated),
    )?;
    wait_until(|| {
        assembly
            .transport
            .relay(&url, &Authority::Unauthenticated)
            .is_some()
    })
    .await;
    let public_peer = assembly
        .transport
        .relay(&url, &Authority::Unauthenticated)
        .unwrap();

    wait_until(|| requests(Some(public_peer.clone())).len() == 1).await;
    wait_until(|| requests(Some(private_peer.clone())).len() == 1).await;
    let public_wire = requests(Some(public_peer.clone()))[0].0.clone();
    let private_wire = requests(Some(private_peer.clone()))[0].0.clone();
    assert_ne!(public_wire, private_wire);

    Ok(Setup {
        assembly,
        url,
        public,
        private,
        public_peer,
        public_wire,
    })
}

#[tokio::test(flavor = "current_thread")]
async fn exact_access_keys_isolate_event_eose_and_challenge()
-> Result<(), Box<dyn std::error::Error>> {
    let Setup {
        assembly: _assembly,
        url,
        public,
        private,
        public_peer,
        public_wire,
    } = setup().await?;

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
            .relay(&url)
            .is_some_and(fava_query::RelayQueryEvidence::stored_events_complete)
    })
    .await;
    assert!(
        !private
            .current()
            .evidence
            .relay(&url)
            .unwrap()
            .stored_events_complete()
    );

    // A relay may challenge a connection that asked for nothing. This
    // observation wants public access, so a demand on its session is not its
    // fact -- it is not waiting on an answer and never was. Neither
    // observation changes.
    push(&public_peer, &RelayMessage::auth("public challenge"));
    settle().await;
    assert!(
        matches!(
            public
                .current()
                .evidence
                .relay(&url)
                .map(|item| &item.state),
            Some(RelaySourceState::StoredEventsComplete { .. })
        ),
        "an anonymous observation is not waiting on a challenge it never provoked, and its \
         completed window is not overwritten by one, got {:?}",
        public
            .current()
            .evidence
            .relay(&url)
            .map(|item| item.state.clone())
    );
    assert!(matches!(
        private
            .current()
            .evidence
            .relay(&url)
            .map(|item| &item.state),
        Some(RelaySourceState::Open { .. })
    ));

    public.close();
    private.close();
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn a_close_refusal_on_one_connection_leaves_the_other_untouched()
-> Result<(), Box<dyn std::error::Error>> {
    let Setup {
        assembly: _assembly,
        url,
        public,
        private,
        public_peer,
        public_wire,
    } = setup().await?;

    push(
        &public_peer,
        &RelayMessage::closed(public_wire, "public refused"),
    );
    wait_until(|| {
        matches!(
            public
                .current()
                .evidence
                .relay(&url)
                .map(|item| &item.state),
            Some(RelaySourceState::Refused { .. })
        )
    })
    .await;
    assert!(!matches!(
        private
            .current()
            .evidence
            .relay(&url)
            .map(|item| &item.state),
        Some(RelaySourceState::Refused { .. } | RelaySourceState::AuthenticationRequired { .. })
    ));

    public.close();
    private.close();
    Ok(())
}
