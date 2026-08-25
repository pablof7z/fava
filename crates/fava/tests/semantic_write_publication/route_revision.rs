use std::collections::{BTreeMap, BTreeSet};

use fava::{Fava, Receipt, ReceiptId};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_router_testkit::DelayedRouter;
use fava_routing::{RouteContribution, RouteDestination};
use nostr::types::RelayUrl;

use super::*;

#[tokio::test(flavor = "current_thread")]
async fn delayed_route_after_rematerialization_commits_newer_revision() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let first = signed_source(&keys, Kind::ContactList, 10, "first", &[]);
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            first,
            relay_occurrence(),
        ))])
        .unwrap();
    let initial = RelayUrl::parse("wss://initial-route.example").unwrap();
    let delayed = Arc::new(DelayedRouter::new(
        "semantic-revision",
        contribution(initial),
    ));
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .router(Arc::clone(&delayed))
    .materializer(materializer)
    .build()
    .unwrap();
    let write = fava
        .by(keys.public_key())
        .publish(edit(Kind::ContactList))
        .unwrap();
    wait_for_signer(&signer, 1).await;

    let successor = signed_source(&keys, Kind::ContactList, 20, "successor", &[]);
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            successor,
            relay_occurrence(),
        ))])
        .unwrap();
    wait_for_materialization(&fava, write.receipt_id(), 2).await;
    wait_for_signer(&signer, 2).await;
    let reopened = wait_for_route_revision(&fava, write.receipt_id(), 3).await;

    let later = RelayUrl::parse("wss://later-route.example").unwrap();
    let later_session = RelaySessionKey {
        relay: later.clone(),
        access: RelayAccess::Public,
    };
    delayed.replace(contribution(later));
    let updated = wait_for_destination(&fava, write.receipt_id(), &later_session).await;
    assert!(updated.route_revision > reopened.route_revision);
}

fn contribution(relay: RelayUrl) -> RouteContribution {
    RouteContribution {
        destinations: vec![RouteDestination::new(
            RelaySessionKey {
                relay,
                access: RelayAccess::Public,
            },
            BTreeSet::new(),
            "controlled semantic route",
        )],
        coverage: BTreeMap::new(),
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
    }
}

async fn wait_for_route_revision(fava: &Fava, receipt_id: ReceiptId, revision: u64) -> Receipt {
    wait_for_receipt(fava, receipt_id, |receipt| {
        receipt.route_revision >= revision
    })
    .await
}

async fn wait_for_destination(
    fava: &Fava,
    receipt_id: ReceiptId,
    destination: &RelaySessionKey,
) -> Receipt {
    wait_for_receipt(fava, receipt_id, |receipt| {
        receipt.destinations().contains_key(destination)
    })
    .await
}

async fn wait_for_receipt(
    fava: &Fava,
    receipt_id: ReceiptId,
    predicate: impl Fn(&Receipt) -> bool,
) -> Receipt {
    tokio::time::timeout(Duration::from_secs(1), async {
        let mut changes = fava.receipt_changes();
        loop {
            let receipt = fava.receipt(receipt_id).unwrap().unwrap();
            if predicate(&receipt) {
                return receipt;
            }
            changes.recv().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| panic!("receipt did not reach controlled route state"))
}
