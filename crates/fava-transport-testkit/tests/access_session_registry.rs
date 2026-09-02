//! The fake transport registry opens one connection per relay, and a second
//! only once the first can no longer reach what is being asked of it.

use std::num::NonZeroUsize;
use std::time::Duration;

use fava_relay::{Authentication, Authority};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, RelaySessionExt, Transport,
    TransportBounds, TransportDeadlines,
};
use fava_transport_testkit::FakeTransport;
use nostr::key::Keys;
use nostr::types::RelayUrl;

fn request(relay: RelayUrl, authority: Authority) -> OpenRelaySession {
    OpenRelaySession {
        relay,
        authority,
        deadlines: TransportDeadlines {
            establish: Duration::from_millis(100),
            write: Duration::from_millis(100),
            idle: Duration::from_secs(1),
            close: Duration::from_millis(100),
        },
        bounds: TransportBounds {
            inbound_frames: NonZeroUsize::new(8).unwrap(),
            outbound_frames: NonZeroUsize::new(8).unwrap(),
            max_frame_bytes: NonZeroUsize::new(1_024).unwrap(),
        },
        reconnect_attempts: Some(NonZeroUsize::new(2).unwrap()),
    }
}

/// Nothing has told the relay who is asking yet, so a fresh connection can
/// still become anyone's: public and authenticated demand for the same relay
/// share it rather than opening two sockets.
#[tokio::test(flavor = "current_thread")]
async fn a_fresh_connection_can_still_become_anyones() -> Result<(), Box<dyn std::error::Error>> {
    let relay = RelayUrl::parse("wss://relay.example")?;
    let alice = Keys::generate().public_key();
    let transport = FakeTransport::new();
    let public_lease = transport
        .acquire_session(request(relay.clone(), Authority::Unauthenticated))
        .await?;
    let private_lease = transport
        .acquire_session(request(relay.clone(), Authority::As(alice)))
        .await?;
    assert_eq!(
        public_lease.session().identity(),
        private_lease.session().identity(),
        "an unasked connection reaches every authority"
    );
    assert_eq!(transport.dials(&relay), 1);
    Ok(())
}

/// Once a connection is committed to one account, it can never become
/// anyone else's or become anonymous again: a distinct request opens a
/// distinct connection, and every other undecided request keeps sharing what
/// remains reachable.
#[tokio::test(flavor = "current_thread")]
async fn an_authenticated_connection_cannot_become_anyone_elses()
-> Result<(), Box<dyn std::error::Error>> {
    let relay = RelayUrl::parse("wss://relay.example")?;
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let transport = FakeTransport::new();

    let alice_lease = transport
        .acquire_session(request(relay.clone(), Authority::As(alice)))
        .await?;
    RelaySessionExt::record_authentication(
        alice_lease.session(),
        Authentication::Authenticated { as_of: alice },
    );

    let bob_lease = transport
        .acquire_session(request(relay.clone(), Authority::As(bob)))
        .await?;
    let anonymous_lease = transport
        .acquire_session(request(relay.clone(), Authority::Unauthenticated))
        .await?;

    assert_ne!(
        alice_lease.session().identity(),
        bob_lease.session().identity(),
        "alice's connection cannot become bob's"
    );
    assert_ne!(
        alice_lease.session().identity(),
        anonymous_lease.session().identity(),
        "alice's connection cannot become anonymous again"
    );
    assert_eq!(
        bob_lease.session().identity(),
        anonymous_lease.session().identity(),
        "bob and anonymous work still share the one remaining undecided connection"
    );
    assert_eq!(transport.dials(&relay), 2);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn same_correlation_completions_remain_scoped_to_exact_relay_and_generation()
-> Result<(), Box<dyn std::error::Error>> {
    let relay = RelayUrl::parse("wss://relay.example")?;
    let alice = Keys::generate().public_key();
    let transport = FakeTransport::new();

    let alice_lease = transport
        .acquire_session(request(relay.clone(), Authority::As(alice)))
        .await?;
    RelaySessionExt::record_authentication(
        alice_lease.session(),
        Authentication::Authenticated { as_of: alice },
    );
    let anonymous_lease = transport
        .acquire_session(request(relay.clone(), Authority::Unauthenticated))
        .await?;
    assert_ne!(
        alice_lease.session().identity(),
        anonymous_lease.session().identity()
    );

    let alice_peer = transport
        .relay(&relay, &Authority::As(alice))
        .expect("alice's connection");
    let anonymous_peer = transport
        .relay(&relay, &Authority::Unauthenticated)
        .expect("the remaining anonymous connection");
    alice_peer.stall_writer();
    anonymous_peer.stall_writer();
    let correlation = HandoffCorrelation::new(7);
    assert!(matches!(
        alice_lease
            .session()
            .hand_off(b"alice".to_vec(), correlation)
            .await,
        HandoffOutcome::HandedOff { .. }
    ));
    assert!(matches!(
        anonymous_lease
            .session()
            .hand_off(b"anonymous".to_vec(), correlation)
            .await,
        HandoffOutcome::HandedOff { .. }
    ));
    let alice_generation = alice_lease.session().identity();
    let anonymous_generation = anonymous_lease.session().identity();

    alice_peer.reconnect();

    let alice_completion = alice_peer
        .unflushed_completions()
        .into_iter()
        .next()
        .expect("alice's in-flight completion");
    assert_eq!(alice_completion.identity(), &alice_generation);
    assert_eq!(alice_completion.correlation(), correlation);
    assert!(anonymous_peer.unflushed_completions().is_empty());
    assert_eq!(anonymous_lease.session().identity(), anonymous_generation);

    anonymous_peer.fail_now("anonymous failure");
    let anonymous_completion = anonymous_peer
        .unflushed_completions()
        .into_iter()
        .next()
        .expect("anonymous in-flight completion");
    assert_eq!(anonymous_completion.identity(), &anonymous_generation);
    assert_eq!(anonymous_completion.correlation(), correlation);
    assert_ne!(alice_completion.identity(), anonymous_completion.identity());
    Ok(())
}
