//! Ten ordinary-application flows driven through the public Fava facade.
//!
//! This module is the canary's consumer surface. It may depend only on the
//! `fava` facade and on provider crates an application would select. It must
//! never reach into a Fava internal crate, never install a stub transport,
//! publisher, or write store, never construct a second engine to feed the
//! first, and never hand-feed data the library should have acquired.
//!
//! When a flow cannot be written that way, the flow is recorded as a wall and
//! left failing. A wall is the deliverable, not a defect in this file.

use std::fmt::Write as _;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fava::{EventBuilder, Fava, Kind, Observation, PublicKey, Query, RelayUrl, all};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_router_app_relays::AppRelayRouter;
use fava_signer_local::LocalSigner;
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::artifacts::RunArtifacts;
use crate::proxy::WireProxy;
use crate::{CanaryError, CanaryResult, deterministic_keys, wire};

/// A relay address that accepts TCP but is not routable: connections hang.
const BLACKHOLE_RELAY: &str = "ws://192.0.2.1:8080";

/// Every flow gets this much wall-clock budget before it is called frozen.
const RESPONSIVE_BUDGET: Duration = Duration::from_secs(5);

/// Inputs for the consumer flow suite.
#[derive(Clone, Debug)]
pub struct FlowOptions {
    /// A reachable real relay, for example `ws://127.0.0.1:7447`.
    pub relay_url: String,
    /// Caller-selected seed used to derive disposable identities.
    pub seed: String,
    /// Parent directory for preserved evidence bundles.
    pub runs_directory: PathBuf,
}

/// One flow result as an outside developer would report it.
#[derive(Clone, Debug)]
struct FlowRecord {
    id: &'static str,
    intent: &'static str,
    status: &'static str,
    severity: &'static str,
    conclusion: String,
    detail: Value,
}

impl FlowRecord {
    fn passed(id: &'static str, intent: &'static str, detail: Value) -> Self {
        Self {
            id,
            intent,
            status: "passed",
            severity: "none",
            conclusion: String::new(),
            detail,
        }
    }

    fn wall(
        id: &'static str,
        intent: &'static str,
        severity: &'static str,
        conclusion: impl Into<String>,
        detail: Value,
    ) -> Self {
        Self {
            id,
            intent,
            status: "wall",
            severity,
            conclusion: conclusion.into(),
            detail,
        }
    }

    fn json(&self) -> Value {
        json!({
            "flow": self.id,
            "intent": self.intent,
            "status": self.status,
            "severity": self.severity,
            "conclusion": self.conclusion,
            "detail": self.detail,
        })
    }
}

/// Run the ten consumer flows against one reachable real relay.
///
/// # Errors
///
/// Returns an error when evidence cannot be persisted, or when at least one
/// flow hit a wall. A wall is a reported defect, so the run exits nonzero.
pub async fn run_flows_scenario(options: FlowOptions) -> CanaryResult<PathBuf> {
    let mut artifacts = RunArtifacts::create(&options.runs_directory, "dx-flows", &options.seed)?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": "dx-flows", "relay": options.relay_url }),
    )?;

    let upstream = crate::relay_socket_address(&options.relay_url)?;
    let proxy = WireProxy::start(upstream, &artifacts.root().join("wire/flows.jsonl")).await?;
    let live = RelayUrl::parse(&proxy.url()).map_err(error)?;
    let unreachable = RelayUrl::parse(&format!("ws://127.0.0.1:{}", closed_port().await?))
        .map_err(error)?;
    let blackhole = RelayUrl::parse(BLACKHOLE_RELAY).map_err(error)?;

    let mut records = Vec::new();
    records.push(flow_01_engine_before_account(&live).await);
    records.push(flow_02_offline_local_view(&unreachable, &blackhole).await);
    records.push(flow_03_runtime_signer(&live, &options.seed));
    records.push(flow_04_two_accounts(&live, &options.seed).await);
    records.push(flow_05_profile_and_contacts(&live, &options.seed).await);
    records.push(flow_06_automatic_note(&live, &options.seed).await);
    records.push(flow_07_two_observations_one_connection(&live, &proxy).await);
    records.push(flow_08_mixed_relay_health(&live, &unreachable).await);
    records.push(flow_09_cancel_before_delivery(&unreachable, &options.seed));
    records.push(flow_10_clean_close(&options.relay_url).await);

    proxy.shutdown().await?;

    let table: Vec<Value> = records.iter().map(FlowRecord::json).collect();
    artifacts.write_json("flows.json", &json!({ "flows": table }))?;
    let mut report = String::from("# Consumer flow results\n\n");
    for record in &records {
        let note = if record.conclusion.is_empty() {
            String::new()
        } else {
            format!(" -- {}", record.conclusion)
        };
        let _ = writeln!(
            report,
            "- {} [{}] {}{note}",
            record.id, record.status, record.intent
        );
    }
    artifacts.write_report(&report)?;
    artifacts.record("scenario_finished", json!({ "flows": table }))?;

    let walls: Vec<&str> = records
        .iter()
        .filter(|record| record.status == "wall")
        .map(|record| record.id)
        .collect();
    if walls.is_empty() {
        Ok(artifacts.root().to_path_buf())
    } else {
        Err(CanaryError::new(format!(
            "consumer flows hit walls: {} (evidence: {})",
            walls.join(", "),
            artifacts.root().display()
        )))
    }
}

// ---------------------------------------------------------------------------
// Assembly an ordinary application writes once at start-up.
// ---------------------------------------------------------------------------

/// Assemble a read-only engine: no account exists yet, so no signer exists.
fn read_only_engine(relays: &[RelayUrl]) -> CanaryResult<Fava> {
    let mut builder = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(StandardSubscriptionPlanner::default()))
        .transport(Arc::new(WebSocketTransport::default()));
    if !relays.is_empty() {
        builder = builder.router(Arc::new(AppRelayRouter::new(
            "app-relays",
            relays.iter().cloned(),
        )));
    }
    builder.build().map_err(error)
}

/// Assemble a publishing engine for accounts already known at start-up.
///
/// Every signer must be named here. There is no later door.
fn publishing_engine(relays: &[RelayUrl], accounts: &[Keys]) -> CanaryResult<Fava> {
    let mut builder = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(StandardSubscriptionPlanner::default()))
        .transport(Arc::new(WebSocketTransport::default()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .materializers([fava_nip02::materializer()]);
    if !relays.is_empty() {
        builder = builder.router(Arc::new(AppRelayRouter::new(
            "app-relays",
            relays.iter().cloned(),
        )));
    }
    for account in accounts {
        builder = builder.signer(Arc::new(LocalSigner::new(account.clone())));
    }
    builder.build().map_err(error)
}

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
    let query = Query::events().kind(Kind::TextNote);
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
        .kind(Kind::TextNote)
        .from_relays([unreachable.clone()])
    {
        Ok(query) => query,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), detail);
        }
    };
    let started = Instant::now();
    let explicit_outcome = match tokio::time::timeout(RESPONSIVE_BUDGET, explicit.observe(query))
        .await
    {
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
                engine.observe(Query::events().kind(Kind::TextNote)),
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
                engine.observe(Query::events().kind(Kind::TextNote)),
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

    // The application starts before its user has an account.
    let engine = match read_only_engine(std::slice::from_ref(live)) {
        Ok(engine) => engine,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };

    // The user now creates an account.
    let account = match deterministic_keys(&format!("{seed}-runtime-account")) {
        Ok(keys) => keys,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };

    // The line an application needs here does not exist:
    //
    //     engine.add_signer(Arc::new(LocalSigner::new(account.clone())));
    //
    // `Fava` exposes no signer, account, or session mutation of any kind.
    // `FavaBuilder::signer` is consumed by `build`, and `Publication` stores
    // its signers in an immutable `BTreeMap<PublicKey, Arc<dyn Signer>>`.
    // There is no interior mutability and no re-assembly door.
    let note = match EventBuilder::new(account.public_key(), Kind::TextNote)
        .content(format!("Fava runtime-signer flow {seed}"))
        .build()
    {
        Ok(note) => note,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let refusal = match engine.publish(note) {
        Ok(_) => {
            return FlowRecord::wall(
                ID,
                INTENT,
                "major",
                "publication was accepted by an engine that has no signer for the author",
                json!({ "accepted_without_signer": true }),
            );
        }
        Err(error) => error.to_string(),
    };

    FlowRecord::wall(
        ID,
        INTENT,
        "show-stopper",
        "no public API attaches a signer to a running engine; the account can only \
         be used by assembling a new Fava, which is an application restart",
        json!({
            "attempted_call": "Fava::add_signer(Arc<LocalSigner>)",
            "api_exists": false,
            "publish_refusal": refusal,
            "signer_registration_sites": ["FavaBuilder::signer", "FavaBuilder::signers"],
            "both_consumed_by": "FavaBuilder::build",
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
    let engine = match publishing_engine(
        std::slice::from_ref(live),
        &[first.clone(), second.clone()],
    ) {
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

#[allow(clippy::too_many_lines, reason = "one flow reads end to end as one story")]
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
    let profile = EventBuilder::new(me.public_key(), Kind::Metadata)
        .content(format!("{{\"name\":\"fava-flow-{seed}\"}}"))
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
    let profile_query = Query::events()
        .kind(Kind::Metadata)
        .authors([me.public_key()]);
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
    let contact_query = fava_nip02::contact_list(me.public_key());
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
    detail["follows_after_follow"] = json!(follows.iter().map(PublicKey::to_hex).collect::<Vec<_>>());
    if !follows.contains(&target.public_key()) {
        return FlowRecord::wall(
            ID,
            INTENT,
            "show-stopper",
            "the followed key is absent from the materialized contact list",
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
            "the unfollowed key is still present in the materialized contact list",
            detail,
        );
    }
    FlowRecord::passed(ID, INTENT, detail)
}

// ---------------------------------------------------------------------------
// Flow 6: publish a note to automatically-routed relays.
// ---------------------------------------------------------------------------

async fn flow_06_automatic_note(live: &RelayUrl, seed: &str) -> FlowRecord {
    const ID: &str = "flow-06-automatic-note";
    const INTENT: &str = "publish a note to automatically-routed relays";

    let account = match deterministic_keys(&format!("{seed}-auto-note")) {
        Ok(keys) => keys,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let engine = match publishing_engine(std::slice::from_ref(live), std::slice::from_ref(&account)) {
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

async fn flow_07_two_observations_one_connection(
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
    let query = Query::events().kind(Kind::TextNote);
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
            return FlowRecord::wall(ID, INTENT, "show-stopper", "second observe froze", json!({}));
        }
    };
    tokio::time::sleep(Duration::from_millis(250)).await;
    let opened = proxy.connection_count().saturating_sub(before);
    let sessions = engine.diagnostics().sessions.len();
    let subscriptions = engine.diagnostics().subscriptions.len();
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

#[allow(clippy::too_many_lines, reason = "one flow reads end to end as one story")]
async fn flow_08_mixed_relay_health(live: &RelayUrl, down: &RelayUrl) -> FlowRecord {
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
        .kind(Kind::TextNote)
        .from_relays([live.clone(), down.clone()])
    {
        Ok(query) => query,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), detail);
        }
    };
    let started = Instant::now();
    detail["explicit_relays"] = match tokio::time::timeout(RESPONSIVE_BUDGET, explicit.observe(query))
        .await
    {
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
        automatic.observe(Query::events().kind(Kind::TextNote)),
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
        .sessions
        .iter()
        .map(|(session, generation)| format!("{} gen {generation}", session.relay))
        .collect();
    let shortfalls: Vec<String> = diagnostics
        .route_shortfalls
        .iter()
        .map(|(revision, message)| format!("revision {revision}: {message}"))
        .collect();
    let failures: Vec<String> = diagnostics
        .failures
        .iter()
        .map(|(session, _, message)| format!("{}: {message}", session.relay))
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

fn flow_09_cancel_before_delivery(down: &RelayUrl, seed: &str) -> FlowRecord {
    const ID: &str = "flow-09-cancel-before-delivery";
    const INTENT: &str = "cancel a write before it is delivered";

    let account = match deterministic_keys(&format!("{seed}-cancel")) {
        Ok(keys) => keys,
        Err(error) => {
            return FlowRecord::wall(ID, INTENT, "show-stopper", error.to_string(), json!({}));
        }
    };
    let engine = match publishing_engine(std::slice::from_ref(down), std::slice::from_ref(&account)) {
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

async fn flow_10_clean_close(relay_url: &str) -> FlowRecord {
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
        .observe(Query::events().kind(Kind::TextNote))
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

// ---------------------------------------------------------------------------
// Shared helpers. None of these substitute for Fava behaviour.
// ---------------------------------------------------------------------------

async fn publish_note(engine: &Fava, author: PublicKey, label: &str) -> CanaryResult<String> {
    let note = EventBuilder::new(author, Kind::TextNote)
        .content(format!("Fava flow note {label}"))
        .build()
        .map_err(error)?;
    publish_and_settle(engine, note).await
}

async fn publish_and_settle(engine: &Fava, note: fava::UnsignedEvent) -> CanaryResult<String> {
    settle(engine.publish(note)).await
}

async fn settle(
    write: Result<fava::Write, fava::PublishError>,
) -> CanaryResult<String> {
    let write = write.map_err(error)?;
    let receipt = tokio::time::timeout(Duration::from_secs(10), write.settled(all()))
        .await
        .map_err(|_| CanaryError::new("timed out awaiting a terminal receipt"))?
        .map_err(error)?;
    Ok(receipt.current.id().to_hex())
}

/// Open an observation, wait for at least `expected` records, then close.
async fn read_back(
    engine: &Fava,
    query: Query,
    relay: &RelayUrl,
    expected: usize,
) -> CanaryResult<usize> {
    let query = query
        .from_relays([relay.clone()])
        .map_err(error)?;
    let mut observation = tokio::time::timeout(RESPONSIVE_BUDGET, engine.observe(query))
        .await
        .map_err(|_| CanaryError::new("observe froze"))?
        .map_err(error)?;
    let count = wait_for(&mut observation, expected).await;
    observation.close();
    count
}

/// Read current local state for a query without creating relay demand.
async fn observe_local(engine: &Fava, query: Query) -> CanaryResult<fava::QuerySnapshot> {
    let observation = tokio::time::timeout(RESPONSIVE_BUDGET, engine.observe(query.cache_only()))
        .await
        .map_err(|_| CanaryError::new("cache-only observe froze"))?
        .map_err(error)?;
    let snapshot = observation.current();
    observation.close();
    Ok((*snapshot).clone())
}

async fn wait_for(observation: &mut Observation, expected: usize) -> CanaryResult<usize> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let count = observation.current().events.len();
            if count >= expected {
                return Ok(count);
            }
            observation
                .changed()
                .await
                .map_err(|_| CanaryError::new("observation closed before the expected records"))?;
        }
    })
    .await
    .map_err(|_| CanaryError::new("timed out awaiting query records"))?
}

async fn closed_port() -> CanaryResult<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
