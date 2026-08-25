use std::future::Future;
use std::time::Duration;

use fava::{Fava, MaterializationId, Observation, Query, Receipt, ReceiptId, RelayUrl, Write, all};
use fava_external_semantic_capability_proof::external_query;

const DEADLINE: Duration = Duration::from_secs(2);

pub(super) async fn with_deadline<T>(
    label: &str,
    last_state: impl FnOnce() -> String,
    future: impl Future<Output = T>,
) -> T {
    match tokio::time::timeout(DEADLINE, future).await {
        Ok(value) => value,
        Err(_) => panic!(
            "{label} exceeded {DEADLINE:?}; last state: {}",
            last_state()
        ),
    }
}

pub async fn open_observation(fava: &Fava, query: Query, label: &str) -> Observation {
    let query_state = format!("query={query:?}");
    with_deadline(label, || query_state, fava.observe(query))
        .await
        .expect("public query opens")
}

pub async fn open_external_source(
    fava: &Fava,
    relay: &RelayUrl,
    actor: fava::PublicKey,
) -> Observation {
    let query = external_query(actor)
        .from_relays([relay.clone()])
        .expect("one explicit relay");
    open_observation(fava, query, "external source observation open").await
}

pub async fn wait_receipt(
    fava: &Fava,
    receipt_id: ReceiptId,
    predicate: impl Fn(&Receipt) -> bool,
) -> Receipt {
    let mut changes = fava.receipt_changes();
    with_deadline(
        "receipt transition",
        || format!("receipt={:?}", fava.receipt(receipt_id)),
        async {
            loop {
                if let Some(receipt) = fava.receipt(receipt_id).expect("receipt reads")
                    && predicate(&receipt)
                {
                    return receipt;
                }
                if let Ok((changed, Some(receipt))) = changes.recv().await
                    && changed == receipt_id
                    && predicate(&receipt)
                {
                    return receipt;
                }
            }
        },
    )
    .await
}

pub async fn wait_generation_record(
    observation: &mut Observation,
    generation: u64,
) -> fava::EventRecord {
    let result = tokio::time::timeout(DEADLINE, async {
        loop {
            let snapshot = observation.current();
            if let Some(record) = snapshot.events.iter().find(|record| {
                record.publication().is_some_and(|evidence| {
                    evidence.materialization_id == MaterializationId::from_u64(generation)
                })
            }) {
                return record.clone();
            }
            observation
                .changed()
                .await
                .expect("observation remains live");
        }
    })
    .await;
    match result {
        Ok(record) => record,
        Err(_) => panic!(
            "query generation {generation} exceeded {DEADLINE:?}; last state: {:?}",
            observation.current()
        ),
    }
}

pub async fn wait_first_record(observation: &mut Observation, label: &str) -> fava::EventRecord {
    let result = tokio::time::timeout(DEADLINE, async {
        loop {
            if let Some(record) = observation.current().events.first() {
                return record.clone();
            }
            observation
                .changed()
                .await
                .expect("observation remains live");
        }
    })
    .await;
    match result {
        Ok(record) => record,
        Err(_) => panic!(
            "{label} exceeded {DEADLINE:?}; last state: {:?}",
            observation.current()
        ),
    }
}

pub async fn wait_eose(fava: &Fava, subscription: &str) {
    with_deadline(
        "EOSE processing",
        || format!("diagnostics={:?}", fava.diagnostics()),
        async {
            loop {
                if fava
                    .diagnostics()
                    .relays
                    .iter()
                    .flat_map(|relay| relay.subscriptions.iter())
                    .any(|wire| wire.id.as_str() == subscription && wire.stored_events_complete)
                {
                    return;
                }
                tokio::task::yield_now().await;
            }
        },
    )
    .await;
}

pub async fn wait_terminal(write: &Write, label: &str) -> Receipt {
    with_deadline(
        label,
        || format!("receipt={:?}", write.receipt()),
        write.settled(all()),
    )
    .await
    .expect("receipt reaches terminal state")
}
