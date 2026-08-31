//! The fake transport registry isolates same-URL sessions by exact access.

use std::num::NonZeroUsize;
use std::time::Duration;

use fava_relay::{RelayAccess, RelaySessionKey};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, Transport, TransportBounds,
    TransportDeadlines,
};
use fava_transport_testkit::FakeTransport;
use nostr::key::Keys;
use nostr::types::RelayUrl;

fn request(key: RelaySessionKey) -> OpenRelaySession {
    OpenRelaySession {
        key,
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

#[tokio::test(flavor = "current_thread")]
async fn same_url_public_and_authenticated_open_distinct_generations()
-> Result<(), Box<dyn std::error::Error>> {
    let relay = RelayUrl::parse("wss://relay.example")?;
    let public = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    };
    let authenticated = RelaySessionKey {
        relay,
        access: RelayAccess::Authenticated(Keys::generate().public_key()),
    };
    let transport = FakeTransport::new();
    let public_lease = transport.acquire_session(request(public.clone())).await?;
    let private_lease = transport
        .acquire_session(request(authenticated.clone()))
        .await?;
    let public_identity = public_lease.session().identity();
    let private_identity = private_lease.session().identity();
    assert_eq!(public_identity.key, public);
    assert_eq!(private_identity.key, authenticated);
    assert_ne!(public_identity, private_identity);
    assert_eq!(transport.dials(&public_identity.key), 1);
    assert_eq!(transport.dials(&private_identity.key), 1);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn same_correlation_completions_remain_scoped_to_exact_key_and_generation()
-> Result<(), Box<dyn std::error::Error>> {
    let relay = RelayUrl::parse("wss://relay.example")?;
    let public = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    };
    let authenticated = RelaySessionKey {
        relay,
        access: RelayAccess::Authenticated(Keys::generate().public_key()),
    };
    let transport = FakeTransport::new();
    let public_lease = transport.acquire_session(request(public.clone())).await?;
    let private_lease = transport
        .acquire_session(request(authenticated.clone()))
        .await?;
    let public_peer = transport.relay(&public).expect("public session");
    let private_peer = transport
        .relay(&authenticated)
        .expect("authenticated session");
    public_peer.stall_writer();
    private_peer.stall_writer();
    let correlation = HandoffCorrelation::new(7);
    assert!(matches!(
        public_lease
            .session()
            .hand_off(b"public".to_vec(), correlation)
            .await,
        HandoffOutcome::HandedOff { .. }
    ));
    assert!(matches!(
        private_lease
            .session()
            .hand_off(b"private".to_vec(), correlation)
            .await,
        HandoffOutcome::HandedOff { .. }
    ));
    let public_generation = public_lease.session().identity();
    let private_generation = private_lease.session().identity();

    public_peer.reconnect();

    let public_completion = public_peer
        .unflushed_completions()
        .into_iter()
        .next()
        .expect("public in-flight completion");
    assert_eq!(public_completion.identity(), &public_generation);
    assert_eq!(public_completion.correlation(), correlation);
    assert!(private_peer.unflushed_completions().is_empty());
    assert_eq!(private_lease.session().identity(), private_generation);

    private_peer.fail_now("private failure");
    let private_completion = private_peer
        .unflushed_completions()
        .into_iter()
        .next()
        .expect("private in-flight completion");
    assert_eq!(private_completion.identity(), &private_generation);
    assert_eq!(private_completion.correlation(), correlation);
    assert_ne!(public_completion.identity(), private_completion.identity());
    Ok(())
}
