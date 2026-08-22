use super::*;

#[tokio::test]
async fn a_hostile_relay_enters_no_state_while_a_healthy_relay_stays_live() {
    let hostile_url = RelayUrl::parse("wss://hostile.example").expect("relay URL");
    let healthy_url = RelayUrl::parse("wss://healthy.example").expect("relay URL");
    let hostile = Arc::new(Script::default());
    let healthy = Arc::new(Script::default());
    let author = Keys::generate();
    let stranger = Keys::generate();

    let cache = Arc::new(MemoryEventCache::default());
    let fava = Fava::builder()
        .event_cache(Arc::clone(&cache))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(ScriptedTransport {
            scripts: BTreeMap::from([
                (hostile_url.clone(), Arc::clone(&hostile)),
                (healthy_url.clone(), Arc::clone(&healthy)),
            ]),
        }))
        .build()
        .expect("assembly builds");

    let query = Query::events()
        .authors([author.public_key()])
        .only_from_relays([hostile_url.clone(), healthy_url.clone()])
        .expect("explicit relays are valid");
    let observation = fava.observe(query).await.expect("live query opens");

    let hostile_id = hostile.subscription().expect("hostile relay was asked");
    let healthy_id = healthy.subscription().expect("healthy relay was asked");

    let mut forged = signed(&author, "signed body", 10);
    "forged after signing".clone_into(&mut forged.content);
    hostile.push(&RelayMessage::event(hostile_id.clone(), forged));
    hostile.push(&RelayMessage::event(
        hostile_id.clone(),
        signed(&stranger, "wrong author", 11),
    ));
    hostile.push_raw("{not json at all");
    hostile.push(&RelayMessage::event(
        SubscriptionId::new("never-requested"),
        signed(&author, "wrong subscription", 12),
    ));
    hostile.push(&RelayMessage::eose(SubscriptionId::new("never-requested")));
    hostile.push(&RelayMessage::notice("this relay is unhappy"));
    hostile.push(&RelayMessage::closed(hostile_id.clone(), "shutting down"));
    hostile.push(&RelayMessage::event(
        hostile_id.clone(),
        signed(&author, "after CLOSED", 13),
    ));

    wait_until(Duration::from_secs(2), || {
        fava.diagnostics()
            .failures
            .iter()
            .any(|(_, _, reason)| reason.contains("after CLOSED"))
    })
    .await;

    let served = signed(&author, "healthy relay event", 20);
    healthy.push(&RelayMessage::event(healthy_id.clone(), served.clone()));
    healthy.push(&RelayMessage::eose(healthy_id.clone()));

    wait_until(Duration::from_secs(2), || {
        !observation.current().events.is_empty()
    })
    .await;

    let snapshot = observation.current();
    assert_eq!(
        snapshot.events.len(),
        1,
        "exactly the healthy relay's event is visible"
    );
    assert_eq!(snapshot.events[0].id(), served.id);
    assert_eq!(snapshot.events[0].relay_evidence.len(), 1);
    assert!(
        snapshot.events[0]
            .relay_evidence
            .observations()
            .all(|observed| observed.session.relay == healthy_url)
    );

    let diagnostics = fava.diagnostics();
    let reasons: Vec<&str> = diagnostics
        .failures
        .iter()
        .map(|(_, _, reason)| reason.as_str())
        .collect();
    for expected in [
        "verification failed",
        "does not match its accepted filter",
        "invalid relay message",
        "unattributed EVENT",
        "unattributed EOSE",
        "relay NOTICE",
        "EVENT after CLOSED",
    ] {
        assert!(
            reasons.iter().any(|reason| reason.contains(expected)),
            "expected a scoped {expected:?} fact, got {reasons:?}"
        );
    }
    assert!(
        diagnostics
            .failures
            .iter()
            .filter(|(_, _, reason)| {
                reason.contains("after CLOSED") || reason.contains("unattributed")
            })
            .all(|(session, _, _)| session.relay == hostile_url)
    );
    assert!(
        diagnostics
            .eose
            .iter()
            .all(|(session, _, _)| session.relay == healthy_url)
    );
    assert_eq!(cache.len().expect("cache is readable"), 1);
    observation.close();
}

#[tokio::test]
async fn a_relay_that_never_sends_eose_is_silence_not_completeness_or_failure() {
    let relay_url = RelayUrl::parse("wss://quiet.example").expect("relay URL");
    let script = Arc::new(Script::default());
    let author = Keys::generate();
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(ScriptedTransport {
            scripts: BTreeMap::from([(relay_url.clone(), Arc::clone(&script))]),
        }))
        .build()
        .expect("assembly builds");

    let query = Query::events()
        .authors([author.public_key()])
        .only_from_relays([relay_url.clone()])
        .expect("explicit relay is valid");
    let observation = fava.observe(query).await.expect("live query opens");
    let subscription = script.subscription().expect("the relay was asked");

    let served = signed(&author, "served without EOSE", 30);
    script.push(&RelayMessage::event(subscription, served.clone()));
    wait_until(Duration::from_secs(2), || {
        !observation.current().events.is_empty()
    })
    .await;

    let diagnostics = fava.diagnostics();
    assert!(diagnostics.eose.is_empty());
    assert!(diagnostics.failures.is_empty());
    assert_eq!(diagnostics.subscriptions.len(), 1);
    assert_eq!(observation.current().events[0].id(), served.id);
    observation.close();
}
