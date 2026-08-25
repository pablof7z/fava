//! Consumer flows six through ten and the clean-close child entry point.

use super::{
    CanaryError, CanaryResult, Duration, EventBuilder, FlowRecord, Instant, Kind, Query,
    RESPONSIVE_BUDGET, RelaySessionState, RelayUrl, WireProxy, all, deterministic_keys, error,
    json, publish_and_settle, publishing_engine, read_only_engine, wire,
};

// ---------------------------------------------------------------------------
// Flow 6: publish a note to automatically-routed relays.
// ---------------------------------------------------------------------------

pub(super) async fn flow_06_automatic_note(live: &RelayUrl, seed: &str) -> FlowRecord {
    const ID: &str = "flow-06-automatic-note";
    const INTENT: &str = "publish a note to automatically-routed relays";

    let account = match deterministic_keys(&format!("{seed}-auto-note")) {
        Ok(keys) => keys,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let engine = match publishing_engine(std::slice::from_ref(live), std::slice::from_ref(&account))
    {
        Ok(engine) => engine,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let note = EventBuilder::new(account.public_key(), Kind::TextNote)
        .content(format!("Fava automatic routing flow {seed}"))
        .build();
    let note = match note {
        Ok(note) => note,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let note_id = note.id;
    let settled = match publish_and_settle(&engine, note).await {
        Ok(id) => id,
        Err(error) => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "show-stopper",
                format!("automatic publication failed: {error}"),
                json!({}),
            );
        }
    };

    // Independent witness: ask the relay directly, outside Fava.
    let witness = match note_id {
        Some(id) => wire::query_exact(live.as_str(), id, "flow-06").await,
        None => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "major",
                "the built event carried no id",
                json!({}),
            );
        }
    };
    let detail = match witness {
        Ok(witness) => json!({
            "event": settled,
            "relay_returned_event": witness.found_event,
            "relay_returned_eose": witness.saw_eose,
            "routing": "AppRelayRouter (application-configured write relays)",
            "outbox_routing": "not exercised: see flow-06-outbox wall",
        }),
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "major", error.to_string(), json!({}));
        }
    };
    if detail["relay_returned_event"] != json!(true) {
        return FlowRecord::wall(
            ID,
            INTENT,
            "show-stopper",
            "the relay never stored the automatically routed note",
            detail,
        );
    }
    FlowRecord::passed(ID, INTENT, detail)
}

// ---------------------------------------------------------------------------
// Flow 7: observe the same query twice, the app should see one connection.
// ---------------------------------------------------------------------------

pub(super) async fn flow_07_two_observations_one_connection(
    live: &RelayUrl,
    proxy: &WireProxy,
) -> FlowRecord {
    const ID: &str = "flow-07-shared-connection";
    const INTENT: &str = "observe the same query twice and see one relay connection";

    let engine = match read_only_engine(std::slice::from_ref(live)) {
        Ok(engine) => engine,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let before = proxy.connection_count();
    let query = Query::events()
        .kinds([Kind::TextNote])
        .expect("one kind is bounded");
    let first = match tokio::time::timeout(RESPONSIVE_BUDGET, engine.observe(query.clone())).await {
        Ok(Ok(observation)) => observation,
        Ok(Err(error)) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
        Err(_) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", "first observe froze", json!({}));
        }
    };
    let second = match tokio::time::timeout(RESPONSIVE_BUDGET, engine.observe(query)).await {
        Ok(Ok(observation)) => observation,
        Ok(Err(error)) => {
            first.close();
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
        Err(_) => {
            first.close();
            return FlowRecord::wall(
                ID,
                INTENT,
                "show-stopper",
                "second observe froze",
                json!({}),
            );
        }
    };
    tokio::time::sleep(Duration::from_millis(250)).await;
    let opened = proxy.connection_count().saturating_sub(before);
    let diagnostics = engine.diagnostics();
    let sessions = diagnostics.relays.len();
    let subscriptions: usize = diagnostics
        .relays
        .iter()
        .map(|relay| relay.subscriptions.len())
        .sum();
    first.close();
    second.close();
    let detail = json!({
        "relay_connections_opened": opened,
        "engine_sessions": sessions,
        "engine_subscriptions": subscriptions,
    });
    if opened <= 1 {
        FlowRecord::passed(ID, INTENT, detail)
    } else {
        FlowRecord::wall(
            ID,
            INTENT,
            "major",
            format!(
                "two observations of the identical query opened {opened} separate relay \
                 connections; Fava opens one WebSocket and one REQ per observation with no \
                 session or subscription sharing, so an application that renders the same \
                 feed in two places pays for it twice"
            ),
            detail,
        )
    }
}

// ---------------------------------------------------------------------------
// Flow 8: one relay up and one down. Stay responsive, tell them apart.
// ---------------------------------------------------------------------------

#[allow(
    clippy::too_many_lines,
    reason = "one flow reads end to end as one story"
)]
pub(super) async fn flow_08_mixed_relay_health(live: &RelayUrl, down: &RelayUrl) -> FlowRecord {
    const ID: &str = "flow-08-mixed-relay-health";
    const INTENT: &str = "attach one relay that is up and one that is down, stay responsive, \
                          and tell the two apart";
    let mut detail = json!({});

    // Explicit acquisition: an application naming both relays on the query.
    let explicit = match read_only_engine(&[]) {
        Ok(engine) => engine,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), detail);
        }
    };
    let query = match Query::events()
        .kinds([Kind::TextNote])
        .and_then(|query| query.from_relays([live.clone(), down.clone()]))
    {
        Ok(query) => query,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), detail);
        }
    };
    let started = Instant::now();
    detail["explicit_relays"] =
        match tokio::time::timeout(RESPONSIVE_BUDGET, explicit.observe(query)).await {
            Ok(Ok(observation)) => {
                observation.close();
                json!({ "result": "observation opened", "ms": started.elapsed().as_millis() })
            }
            Ok(Err(refusal)) => json!({ "result": "refused", "error": refusal.to_string() }),
            Err(_) => json!({ "result": "froze" }),
        };

    // Automatic acquisition: both relays configured on the application router.
    let automatic = match read_only_engine(&[live.clone(), down.clone()]) {
        Ok(engine) => engine,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), detail);
        }
    };
    let started = Instant::now();
    let observation = match tokio::time::timeout(
        RESPONSIVE_BUDGET,
        automatic.observe(
            Query::events()
                .kinds([Kind::TextNote])
                .expect("one kind is bounded"),
        ),
    )
    .await
    {
        Ok(Ok(observation)) => observation,
        Ok(Err(refusal)) => {
            detail["automatic_routing"] = json!({ "result": "refused",
                                                  "error": refusal.to_string() });
            return FlowRecord::wall(
                ID,
                INTENT,
                "show-stopper",
                "one unreachable relay refuses the whole automatically routed query",
                detail,
            );
        }
        Err(_) => {
            detail["automatic_routing"] = json!({ "result": "froze" });
            return FlowRecord::wall(
                ID,
                INTENT,
                "show-stopper",
                "one unreachable relay freezes the whole automatically routed query",
                detail,
            );
        }
    };
    let open_ms = started.elapsed().as_millis();
    tokio::time::sleep(Duration::from_millis(250)).await;
    let diagnostics = automatic.diagnostics();
    observation.close();

    let healthy: Vec<String> = diagnostics
        .relays
        .iter()
        .filter(|relay| matches!(relay.state, RelaySessionState::Open))
        .map(|relay| format!("{} gen {}", relay.session.relay, relay.generation.0))
        .collect();
    let shortfalls: Vec<String> = diagnostics
        .queries
        .iter()
        .flat_map(|query| {
            let revision = query.route_revision;
            query.shortfalls.iter().map(move |text| {
                revision.map_or_else(
                    || format!("unrouted: {}", text.as_str()),
                    |revision| format!("revision {revision}: {}", text.as_str()),
                )
            })
        })
        .collect();
    let failures: Vec<String> = diagnostics
        .relays
        .iter()
        .filter_map(|relay| match &relay.state {
            RelaySessionState::Reconnecting { detail }
            | RelaySessionState::Unreachable { detail } => {
                Some(format!("{}: {}", relay.session.relay, detail.as_str()))
            }
            _ => None,
        })
        .collect();
    detail["automatic_routing"] = json!({
        "result": "observation opened",
        "ms": open_ms,
        "sessions": healthy,
        "route_shortfalls": shortfalls,
        "failures": failures,
    });

    let names_up = healthy.iter().any(|entry| entry.contains(live.as_str()));
    let names_down = shortfalls
        .iter()
        .chain(failures.iter())
        .any(|entry| entry.contains(down.as_str()));
    let explicit_ok = detail["explicit_relays"]["result"] == "observation opened";

    if names_up && names_down && explicit_ok {
        FlowRecord::passed(ID, INTENT, detail)
    } else {
        let mut reasons = Vec::new();
        if !explicit_ok {
            reasons.push(
                "naming a live relay and a dead relay on one query refuses the whole query, \
                 so the live relay is lost with the dead one"
                    .to_owned(),
            );
        }
        if !names_up {
            reasons.push("no fact identifies the relay that is up".to_owned());
        }
        if !names_down {
            reasons.push("no fact identifies the relay that is down".to_owned());
        }
        FlowRecord::wall(ID, INTENT, "show-stopper", reasons.join("; "), detail)
    }
}

// ---------------------------------------------------------------------------
// Flow 9: cancel a write before it is delivered.
// ---------------------------------------------------------------------------

pub(super) fn flow_09_cancel_before_delivery(down: &RelayUrl, seed: &str) -> FlowRecord {
    const ID: &str = "flow-09-cancel-before-delivery";
    const INTENT: &str = "cancel a write before it is delivered";

    let account = match deterministic_keys(&format!("{seed}-cancel")) {
        Ok(keys) => keys,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let engine = match publishing_engine(std::slice::from_ref(down), std::slice::from_ref(&account))
    {
        Ok(engine) => engine,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let note = EventBuilder::new(account.public_key(), Kind::TextNote)
        .content(format!("Fava cancel flow {seed}"))
        .build();
    let note = match note {
        Ok(note) => note,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let write = match engine.publish(note) {
        Ok(write) => write,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let receipt_id = write.receipt_id();
    let cancelled = engine.cancel_publication(receipt_id);
    let detail = json!({
        "receipt": receipt_id.as_u64(),
        "cancel_result": match &cancelled {
            Ok(Some(receipt)) => format!("cancelled: {:?}", receipt.outcome),
            Ok(None) => "ineligible".to_owned(),
            Err(error) => format!("error: {error}"),
        },
        "open_receipts_after": engine
            .open_receipts()
            .map(|receipts| receipts.len())
            .unwrap_or(usize::MAX),
    });
    match cancelled {
        Ok(Some(_)) => FlowRecord::passed(ID, INTENT, detail),
        Ok(None) => FlowRecord::wall(
            ID,
            INTENT,
            "major",
            "cancellation was refused for a write whose only destination is unreachable, \
             and the API gives no way to know when cancellation would be eligible",
            detail,
        ),
        Err(error) => FlowRecord::wall(ID, INTENT, "major", error.to_string(), detail),
    }
}

// ---------------------------------------------------------------------------
// Flow 10: close the engine cleanly. Nothing should hang.
// ---------------------------------------------------------------------------

pub(super) async fn flow_10_clean_close(relay_url: &str) -> FlowRecord {
    const ID: &str = "flow-10-clean-close";
    const INTENT: &str = "close the engine cleanly with nothing left hanging";

    // The only honest test of "nothing hangs" is a whole process that must
    // exit. `Fava` exposes no `close`, `shutdown`, or `Drop` guarantee, so the
    // application can only drop the value and hope.
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "major", error.to_string(), json!({}));
        }
    };
    let started = Instant::now();
    let child = tokio::process::Command::new(executable)
        .arg("flow-close-child")
        .arg("--relay-url")
        .arg(relay_url)
        .output();
    match tokio::time::timeout(Duration::from_secs(20), child).await {
        Ok(Ok(output)) => {
            let elapsed = started.elapsed();
            let detail = json!({
                "exit_status": output.status.code(),
                "elapsed_ms": elapsed.as_millis(),
                "stderr": String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                "close_api": "none: Fava has no close or shutdown method",
            });
            if output.status.success() {
                FlowRecord::wall(
                    ID,
                    INTENT,
                    "minor",
                    "the process does exit after the engine is dropped, but nothing in the \
                     public API says so: there is no Fava::close, no shutdown, and no way to \
                     await in-flight publication work before exiting",
                    detail,
                )
            } else {
                FlowRecord::wall(
                    ID,
                    INTENT,
                    "show-stopper",
                    "the child process did not exit cleanly after dropping the engine",
                    detail,
                )
            }
        }
        Ok(Err(error)) => FlowRecord::wall(ID, INTENT, "major", error.to_string(), json!({})),
        Err(_) => FlowRecord::wall(
            ID,
            INTENT,
            "show-stopper",
            "the process still had not exited 20 seconds after dropping the engine",
            json!({ "close_api": "none" }),
        ),
    }
}

/// Child entry point for the clean-close flow.
///
/// # Errors
///
/// Returns an error when the engine cannot be assembled or used.
pub async fn run_flow_close_child(arguments: Vec<String>) -> CanaryResult<()> {
    let mut relay_url = None;
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| CanaryError::new("flow-close-child requires --relay-url URL"))?;
        if flag == "--relay-url" {
            relay_url = Some(value);
        } else {
            return Err(CanaryError::new(format!("unknown flag {flag}")));
        }
    }
    let relay_url = relay_url.ok_or_else(|| CanaryError::new("flow-close-child needs a relay"))?;
    let relay = RelayUrl::parse(&relay_url).map_err(error)?;
    let account = deterministic_keys("flow-close-child")?;
    let engine = publishing_engine(std::slice::from_ref(&relay), std::slice::from_ref(&account))?;
    let observation = engine
        .observe(Query::events().kinds([Kind::TextNote]).map_err(error)?)
        .await
        .map_err(error)?;
    let note = EventBuilder::new(account.public_key(), Kind::TextNote)
        .content("Fava clean-close flow")
        .build()
        .map_err(error)?;
    let write = engine.publish(note).map_err(error)?;
    let _ = tokio::time::timeout(Duration::from_secs(5), write.settled(all())).await;
    observation.close();
    drop(engine);
    Ok(())
}
