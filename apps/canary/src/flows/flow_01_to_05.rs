// ---------------------------------------------------------------------------
// Flow 1: construct an engine and open a live query before any account exists.
// ---------------------------------------------------------------------------

async fn flow_01_engine_before_account(live: &RelayUrl) -> FlowRecord {
    const ID: &str = "flow-01-query-before-account";
    const INTENT: &str = "construct an engine and open a live query before any account exists";
    let engine = match read_only_engine(std::slice::from_ref(live)) {
        Ok(engine) => engine,
        Err(error) => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "show-stopper",
                "an engine cannot be assembled without an account",
                json!({ "build_error": error.to_string() }),
            );
        }
    };
    let query = Query::events()
        .kinds([Kind::TextNote])
        .expect("one kind is bounded");
    let started = Instant::now();
    match tokio::time::timeout(RESPONSIVE_BUDGET, engine.observe(query)).await {
        Ok(Ok(observation)) => {
            let elapsed = started.elapsed();
            let revision = observation.current().revision.0;
            observation.close();
            FlowRecord::passed(
                ID,
                INTENT,
                json!({ "open_ms": elapsed.as_millis(), "initial_revision": revision }),
            )
        }
        Ok(Err(refusal)) => FlowRecord::wall(
            ID,
            INTENT,
            "show-stopper",
            "an accountless application cannot open a live query",
            json!({ "observe_error": refusal.to_string() }),
        ),
        Err(_) => FlowRecord::wall(
            ID,
            INTENT,
            "show-stopper",
            "observe did not return within the responsiveness budget",
            json!({ "budget_ms": RESPONSIVE_BUDGET.as_millis() }),
        ),
    }
}

// ---------------------------------------------------------------------------
// Flow 2: with every relay unreachable, a query must return a local view now.
// ---------------------------------------------------------------------------

async fn flow_02_offline_local_view(unreachable: &RelayUrl, blackhole: &RelayUrl) -> FlowRecord {
    const ID: &str = "flow-02-offline-local-view";
    const INTENT: &str = "with every relay unreachable, open a query and get a local view now";
    let mut detail = json!({});

    // The way an application names its relays: explicitly, on the query.
    let explicit = match read_only_engine(&[]) {
        Ok(engine) => engine,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), detail);
        }
    };
    let query = match Query::events()
        .kinds([Kind::TextNote])
        .and_then(|query| query.from_relays([unreachable.clone()]))
    {
        Ok(query) => query,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), detail);
        }
    };
    let started = Instant::now();
    let explicit_outcome =
        match tokio::time::timeout(RESPONSIVE_BUDGET, explicit.observe(query)).await {
            Ok(Ok(observation)) => {
                observation.close();
                json!({ "result": "local view returned", "ms": started.elapsed().as_millis() })
            }
            Ok(Err(refusal)) => {
                json!({ "result": "refused", "error": refusal.to_string(),
                    "ms": started.elapsed().as_millis() })
            }
            Err(_) => json!({ "result": "froze", "budget_ms": RESPONSIVE_BUDGET.as_millis() }),
        };
    detail["explicit_relays"] = explicit_outcome.clone();

    // The other way: automatic routing over configured application relays.
    let automatic_outcome = match read_only_engine(std::slice::from_ref(unreachable)) {
        Ok(engine) => {
            let started = Instant::now();
            match tokio::time::timeout(
                RESPONSIVE_BUDGET,
                engine.observe(
                    Query::events()
                        .kinds([Kind::TextNote])
                        .expect("one kind is bounded"),
                ),
            )
            .await
            {
                Ok(Ok(observation)) => {
                    observation.close();
                    json!({ "result": "local view returned", "ms": started.elapsed().as_millis() })
                }
                Ok(Err(refusal)) => json!({ "result": "refused", "error": refusal.to_string() }),
                Err(_) => json!({ "result": "froze", "budget_ms": RESPONSIVE_BUDGET.as_millis() }),
            }
        }
        Err(error) => json!({ "result": "assembly refused", "error": error.to_string() }),
    };
    detail["automatic_routing"] = automatic_outcome.clone();

    // A relay that drops packets rather than refusing: the classic freeze.
    let blackhole_outcome = match read_only_engine(std::slice::from_ref(blackhole)) {
        Ok(engine) => {
            let started = Instant::now();
            match tokio::time::timeout(
                RESPONSIVE_BUDGET,
                engine.observe(
                    Query::events()
                        .kinds([Kind::TextNote])
                        .expect("one kind is bounded"),
                ),
            )
            .await
            {
                Ok(Ok(observation)) => {
                    observation.close();
                    json!({ "result": "local view returned", "ms": started.elapsed().as_millis() })
                }
                Ok(Err(refusal)) => json!({ "result": "refused", "error": refusal.to_string() }),
                Err(_) => json!({ "result": "froze", "budget_ms": RESPONSIVE_BUDGET.as_millis() }),
            }
        }
        Err(error) => json!({ "result": "assembly refused", "error": error.to_string() }),
    };
    detail["blackhole_relay"] = blackhole_outcome.clone();

    let explicit_ok = explicit_outcome["result"] == "local view returned";
    let automatic_ok = automatic_outcome["result"] == "local view returned";
    let blackhole_ok = blackhole_outcome["result"] == "local view returned";
    if explicit_ok && automatic_ok && blackhole_ok {
        FlowRecord::passed(ID, INTENT, detail)
    } else {
        let mut reasons = Vec::new();
        if !explicit_ok {
            reasons.push("a query naming an unreachable relay does not yield a local view");
        }
        if !automatic_ok {
            reasons.push("automatic routing over an unreachable relay does not yield a local view");
        }
        if !blackhole_ok {
            reasons.push("a relay that drops packets blocks observe for the whole connect");
        }
        FlowRecord::wall(ID, INTENT, "show-stopper", reasons.join("; "), detail)
    }
}

// ---------------------------------------------------------------------------
// Flow 3: create an account at runtime and attach its signer. No restart.
// ---------------------------------------------------------------------------

fn flow_03_runtime_signer(live: &RelayUrl, seed: &str) -> FlowRecord {
    const ID: &str = "flow-03-runtime-signer-attach";
    const INTENT: &str = "create an account at runtime, attach its signer, publish, no restart";

    FlowRecord::wall(
        ID,
        INTENT,
        "major",
        "runtime signer attachment exists and its exact-write wakeup is unit-proved, \
         but this canary still lacks the real-relay causal run and wakeup-removal mutant",
        json!({
            "api": "Fava::add_signer(Arc<dyn Signer>)",
            "api_exists": true,
            "unit_proof": "crates/fava/tests/runtime_signers.rs",
            "missing_live_proof": "docs/issues/0041-runtime-signer-real-app-canary.md",
            "required_relay": live.to_string(),
            "scenario_seed_present": !seed.is_empty(),
        }),
    )
}

// ---------------------------------------------------------------------------
// Flow 4: add a second account, switch between them, publish as each.
// ---------------------------------------------------------------------------

async fn flow_04_two_accounts(live: &RelayUrl, seed: &str) -> FlowRecord {
    const ID: &str = "flow-04-two-accounts";
    const INTENT: &str = "add a second account, switch between them, publish as each";

    let first = match deterministic_keys(&format!("{seed}-account-one")) {
        Ok(keys) => keys,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let second = match deterministic_keys(&format!("{seed}-account-two")) {
        Ok(keys) => keys,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };

    // Both accounts have to be known before `build`, which is the flow-03 wall
    // again: a second account added later cannot reach the running engine.
    let engine =
        match publishing_engine(std::slice::from_ref(live), &[first.clone(), second.clone()]) {
            Ok(engine) => engine,
            Err(error) => {
                return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
            }
        };

    let mut published = Vec::new();
    for (label, account) in [("first", &first), ("second", &second)] {
        match publish_note(&engine, account.public_key(), &format!("{seed}-{label}")).await {
            Ok(id) => published.push(json!({ "account": label, "event": id })),
            Err(error) => {
                return FlowRecord::wall(
                    ID,
                    INTENT,
                    "show-stopper",
                    format!("publishing as the {label} account failed: {error}"),
                    json!({ "published": published }),
                );
            }
        }
    }

    FlowRecord::wall(
        ID,
        INTENT,
        "major",
        "both accounts publish, but only because both were named before build; \
         Fava has no current-account selection, so every call site must carry the \
         author itself and a second account added later cannot be reached",
        json!({
            "published": published,
            "current_account_api": false,
            "runtime_account_addition": false,
        }),
    )
}

// ---------------------------------------------------------------------------
// Flow 5: read a profile and a contact list, follow someone, unfollow.
// ---------------------------------------------------------------------------

#[allow(
    clippy::too_many_lines,
    reason = "one flow reads end to end as one story"
)]
async fn flow_05_profile_and_contacts(live: &RelayUrl, seed: &str) -> FlowRecord {
    const ID: &str = "flow-05-profile-and-contacts";
    const INTENT: &str = "read a profile and a contact list, follow someone, then unfollow";

    let me = match deterministic_keys(&format!("{seed}-contacts-me")) {
        Ok(keys) => keys,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let target = match deterministic_keys(&format!("{seed}-contacts-target")) {
        Ok(keys) => keys,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let engine = match publishing_engine(std::slice::from_ref(live), std::slice::from_ref(&me)) {
        Ok(engine) => engine,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let mut detail = json!({});

    // Publish a profile, then read it back through an ordinary query.
    let profile = EventBuilder::new(Kind::Metadata)
        .content(format!("{{\"name\":\"fava-flow-{seed}\"}}"))
        .by(me.public_key())
        .build();
    let profile = match profile {
        Ok(profile) => profile,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), detail);
        }
    };
    match publish_and_settle(&engine, profile).await {
        Ok(id) => detail["profile_published"] = json!(id),
        Err(error) => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "show-stopper",
                format!("publishing a profile failed: {error}"),
                detail,
            );
        }
    }
    let profile_query = match Query::events()
        .kinds([Kind::Metadata])
        .and_then(|query| query.authors([me.public_key()]))
    {
        Ok(query) => query,
        Err(error) => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "show-stopper",
                format!("constructing the profile query failed: {error}"),
                detail,
            );
        }
    };
    match read_back(&engine, profile_query, live, 1).await {
        Ok(count) => detail["profile_readback"] = json!(count),
        Err(error) => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "major",
                format!("reading the profile back failed: {error}"),
                detail,
            );
        }
    }

    // Follow, then read the contact list back through the NIP-02 provider.
    let follow = match fava_nip02::follow(target.public_key()) {
        Ok(edit) => edit,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), detail);
        }
    };
    let follow_write = engine.by(me.public_key()).publish(follow);
    match settle(follow_write).await {
        Ok(id) => detail["follow_event"] = json!(id),
        Err(error) => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "show-stopper",
                format!("following failed: {error}"),
                detail,
            );
        }
    }
    let contact_query = match fava_nip02::contact_list(me.public_key()) {
        Ok(query) => query,
        Err(error) => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "show-stopper",
                format!("constructing the contact-list query failed: {error}"),
                detail,
            );
        }
    };
    let follows = match observe_local(&engine, contact_query.clone()).await {
        Ok(snapshot) => fava_nip02::follows_of(&snapshot),
        Err(error) => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "major",
                format!("reading the contact list failed: {error}"),
                detail,
            );
        }
    };
    detail["follows_after_follow"] =
        json!(follows.iter().map(PublicKey::to_hex).collect::<Vec<_>>());
    if !follows.contains(&target.public_key()) {
        return FlowRecord::wall(
            ID,
            INTENT,
            "show-stopper",
            "the followed key is absent from the applied contact list",
            detail,
        );
    }

    // Unfollow and confirm the list retracts.
    let unfollow = match fava_nip02::unfollow(target.public_key()) {
        Ok(edit) => edit,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), detail);
        }
    };
    let unfollow_write = engine.by(me.public_key()).publish(unfollow);
    match settle(unfollow_write).await {
        Ok(id) => detail["unfollow_event"] = json!(id),
        Err(error) => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "show-stopper",
                format!("unfollowing failed: {error}"),
                detail,
            );
        }
    }
    let after = match observe_local(&engine, contact_query).await {
        Ok(snapshot) => fava_nip02::follows_of(&snapshot),
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "major", error.to_string(), detail);
        }
    };
    detail["follows_after_unfollow"] =
        json!(after.iter().map(PublicKey::to_hex).collect::<Vec<_>>());
    if after.contains(&target.public_key()) {
        return FlowRecord::wall(
            ID,
            INTENT,
            "show-stopper",
            "the unfollowed key is still present in the applied contact list",
            detail,
        );
    }
    FlowRecord::passed(ID, INTENT, detail)
}

