# Phase 08: Authentication, Hostile Boundaries, and Boundedness - Pattern Map

**Mapped:** 2026-08-21  
**Scope:** Remaining work only  
**Source-state anchor:** `1cdb31e4f431b9f0c3df758ec9c7c7ea2c20f748`  
**Cohesive file groups:** 7  
**Strong or partial analogs:** 6 / 7

## Preservation Boundary

The source state is mixed and must stay intact until its focused owning plan adopts it:

- Twelve tracked source files are modified with 272 insertions and 83 deletions.
- `crates/fava/tests/hostile_ingress.rs` is untracked live WIP, not a file to recreate.
- `stash@{0}` remains `5faecf42c0ec903507e3faeb04962f4680a9cb44` (`autostash`). Never apply, drop, rewrite, or supersede it.
- The source diff is unchanged from the research inventory; current `HEAD` is later only because `1cdb31e` committed the Phase 08 research and validation documents.
- Committed authentication (`ed6a76c`), NIP-11/limit planning (`94e04cd`), and the focused unreachable retry fix (`197c278`) are inputs. Do not plan them as new owner implementations.

Status labels below mean:

- **committed** — present at `HEAD`; extend only for named remaining evidence.
- **dirty WIP** — current working-tree bytes to adopt and reconcile, never regenerate.
- **absent** — research found no implementation or evidence; create only at an existing owner boundary.

## File Classification

### Exact dirty inventory

| File | State | Role | Data flow | Cohesive group | Closest analog | Match |
|---|---|---|---|---|---|---|
| `crates/fava-delivery-standard/src/lib.rs` | dirty WIP | service/policy | event-driven state transition | Delivery lifecycle | `crates/fava-write-store-redb/src/ops.rs` | data-flow match |
| `crates/fava-publisher-nip01/src/lib.rs` | dirty WIP | provider/service | request-response + streaming handoff | Delivery lifecycle | `crates/fava/tests/delivery_bounds.rs` | behavior match |
| `crates/fava-publisher/src/lib.rs` | dirty WIP | model/contract | transform | Delivery lifecycle | `crates/fava-write/src/lib.rs` | exact paired contract |
| `crates/fava-write-store-memory/src/lib.rs` | dirty WIP | store/provider | CRUD + pub-sub | Delivery lifecycle | `crates/fava-write-store-redb/src/ops.rs` | exact provider parity |
| `crates/fava-write-store-memory/src/lifecycle.rs` | dirty WIP | store/service | CRUD state machine | Delivery lifecycle | `crates/fava-write-store-redb/src/ops.rs` | exact provider parity |
| `crates/fava-write-store-memory/src/semantic.rs` | dirty WIP | store/provider | CRUD | Delivery lifecycle | `crates/fava-write-store-redb/src/semantic.rs` | exact provider parity |
| `crates/fava-write-store-redb/src/semantic.rs` | dirty WIP | store/provider | durable CRUD | Delivery lifecycle | `crates/fava-write-store-redb/src/ops.rs` | same-store constructor |
| `crates/fava-write-store/src/receipt.rs` | dirty WIP | utility/validation | transform | Delivery lifecycle | `crates/fava-write-store-redb/src/ops.rs` | contract consumer |
| `crates/fava-write/src/lib.rs` | dirty WIP | model | durable state/serialization | Delivery lifecycle | `crates/fava-write-store-redb/src/ops.rs` | exact consumer |
| `crates/fava-transport-websocket/src/lib.rs` | dirty WIP | provider/transport | streaming | Hostile ingress | `apps/canary/src/hostile.rs` | transport match |
| `crates/fava-transport-websocket/tests/conformance.rs` | dirty WIP | conformance test | streaming | Hostile ingress | `crates/fava-transport-websocket/tests/conformance.rs` existing handoff tests | same-file convention |
| `crates/fava/src/relay.rs` | dirty WIP | controller/session owner | streaming + event-driven | Hostile ingress | `crates/fava/tests/multi_relay.rs` and `apps/canary/src/hostile.rs` | role/data-flow match |
| `crates/fava/tests/hostile_ingress.rs` | untracked WIP | public integration test | streaming/event-driven | Hostile ingress | `crates/fava/tests/multi_relay.rs` | public-Fava scripted match |

### Remaining committed and absent targets

| New/Modified File or Group | State | Role | Data flow | Closest analog | Match quality |
|---|---|---|---|---|---|
| `crates/fava/tests/delivery_bounds.rs` | committed, mixed-tree dependent | public integration test | event-driven lifecycle | `crates/fava-write-store-redb/tests/delivery_lifecycle.rs` | exact behavior, different provider |
| `crates/fava-write-store-redb/tests/delivery_lifecycle.rs` and process/reopen coverage | committed, extend | provider conformance/restart test | durable CRUD + restart | `apps/canary/src/publication.rs:344` | role match |
| `crates/fava/BUILD.bazel` | committed, modify | build config | batch graph | adjacent `rust_test` declarations in the same file | exact |
| `.planning/phases/08-authentication-hostile-boundaries-and-boundedness/08-RESOURCE-LEDGER.md` | absent, inferred filename | config/evidence ledger | batch inventory | no complete analog | none |
| `crates/fava-diagnostics/src/lib.rs` and `tests/relay_facts.rs` | committed, likely modify after ledger | service/model + test | retained event facts | existing `Diagnostics::bounded` | exact role, incomplete behavior |
| Existing owner-local bounds plus `crates/fava/tests/write_bounds.rs` and `observation_bounds.rs` | committed, extend by ledger row | tests/owner services | CRUD, pub-sub, fan-out | current exceed-limit tests | exact pattern |
| `crates/fava/tests/provider_failure_isolation.rs` | absent, inferred filename | public integration/conformance test | request-response + event-driven | `crates/fava/tests/semantic_write_failures.rs` and `fava-publication/src/materialization.rs` | partial |
| Existing provider call sites selected by the ledger | absent as one boundary | controller/service | request-response + cancellation | `fava-publication/src/materialization.rs` | panic-only partial |
| `apps/canary/src/m8.rs` | absent, recommended private module | scenario controller | process/event-driven/request-response | `apps/canary/src/semantic_writes.rs` | exact dispatch pattern |
| `apps/canary/src/m8_child.rs` | absent, recommended private module | process harness | separate-process streaming | `apps/canary/src/publication_child.rs` | role match |
| `apps/canary/src/m8_failure.rs` | absent, recommended private utility | evidence utility | bounded file I/O | `apps/canary/src/semantic_failure.rs` | exact role |
| `apps/canary/src/{artifacts,hostile,proxy,relay,lib,lib_tests,main}.rs` | committed, modify/extend | utilities/providers/config/dispatch | process, streaming, file I/O | same files and M5/M7 modules | exact |
| `apps/canary/scenarios.json` | committed, modify | registry config | batch | existing M0-M7 rows | exact |
| `apps/canary/relays/khatru/main.go` | committed fixture, integrate | external provider/process | HTTP + WebSocket | `apps/canary/src/relay.rs` process supervision | role match |
| `features/relay-authentication.feature`, `features/relay-limits.feature`, and remaining HARD behavior feature text | committed/partly absent | BDD config | batch mapping | `features/semantic-writes.feature` | role match |
| `tools/tests/test_m8_feature_mapping.py` | absent, inferred filename | config-validation test | batch/subprocess | `tools/tests/test_semantic_write_feature.py` | exact role |

The inferred private filenames are planning conveniences, not new architectural vocabulary. A planner may split them differently to remain below the 500-line soft limit, but must retain the responsibilities and existing-owner direction. Do not create a `fava-runtime`, generic common crate, or new public provider contract from this map.

## Pattern Assignments

### Delivery lifecycle group (`HARD-05`, `HARD-06`, `HARD-07`)

**Current state:** mixed. The public test is committed, while the neutral outcome, publisher mapping, policy, spent-budget field, and most Memory/Redb constructor parity remain dirty. Adopt the nine exact dirty files listed above before extending evidence.

**Primary analog:** `crates/fava-write-store-redb/src/ops.rs`  
**Public behavior analog:** `crates/fava/tests/delivery_bounds.rs`  
**Process/restart analog:** `apps/canary/src/publication.rs`

**Separate generation identity from spent budget** (`crates/fava-write/src/lib.rs:507-535`, dirty):

```rust
/// Number of durably authorized attempts per destination.
/// This is attempt generation identity. It only ever increases.
#[serde(with = "attempt_map")]
pub attempts: BTreeMap<RelaySessionKey, u32>,

/// Attempts that actually reached a relay, per destination.
#[serde(default, with = "attempt_map")]
pub spent_attempts: BTreeMap<RelaySessionKey, u32>,

pub fn spent(&self, session: &RelaySessionKey) -> u32 {
    self.spent_attempts.get(session).copied().unwrap_or(0)
}
```

`#[serde(default)]` is the durable compatibility pattern for receipts written before the new field. Do not derive generation from `spent_attempts`.

**Map connection failure before policy evaluation** (`crates/fava-publisher-nip01/src/lib.rs:195-200`, dirty):

```rust
// No connection exists, so no attempt was spent.
Err(error) => {
    return PublishOutcome::Unreachable {
        reason: error.to_string(),
    };
}
```

Keep the paired neutral variants synchronized: `PublishOutcome::Unreachable` at `crates/fava-publisher/src/lib.rs:169-176` maps to `RelayDeliveryOutcome::Unreachable` at `crates/fava-write/src/lib.rs:354-362`.

**Policy consumes spent facts, not elapsed retry generations** (`crates/fava-delivery-standard/src/lib.rs:47-63`, dirty):

```rust
match facts.outcome {
    RelayDeliveryOutcome::Unreachable { .. } => {
        DeliveryDecision::WaitFor(self.unreachable_retry_after)
    }
    RelayDeliveryOutcome::Pending | RelayDeliveryOutcome::Retryable { .. }
        if facts.attempts < self.maximum_attempts.get() =>
    {
        DeliveryDecision::AttemptNow
    }
    // terminal/ceiling arms remain explicit
}
```

**Durable store transition pattern** (`crates/fava-write-store-redb/src/ops.rs:265-315,319-379`, committed):

```rust
let expected_attempt = current_attempt
    .checked_add(1)
    .ok_or_else(|| WriteStoreError::Refused("attempt count exhausted".to_owned()))?;
if attempt != expected_attempt {
    return Err(WriteStoreError::Refused("attempt is not current".to_owned()));
}
// ... exact current materialization/session checks ...
receipt.attempts.insert(session.clone(), attempt);

let spends_budget = matches!(current, RelayDeliveryOutcome::Attempting)
    && !matches!(outcome, RelayDeliveryOutcome::Unreachable { .. });
*current = outcome;
if spends_budget {
    let spent = receipt.spent_attempts.entry(session.clone()).or_default();
    *spent = spent.saturating_add(1);
}
settle(receipt);
```

Copy the exact transition checks and atomic update ordering into Memory parity; do not add a timer-local attempt counter.

**Existing public causal schedule** (`crates/fava/tests/delivery_bounds.rs:183-235`, committed):

```rust
transport.set(UNREACHABLE);
let receipt_id = publish(&fava, &keys);
// wait for exact Unreachable state
assert_eq!(still_parked.spent(&destination()), 0);
assert_eq!(transport.connections.load(Ordering::SeqCst), 0);

transport.set(REFUSES_HANDOFF);
let terminal = tokio::time::timeout(
    Duration::from_secs(5),
    fava.wait_terminal(receipt_id),
).await?;
assert_eq!(terminal.spent(&destination()), 1);
```

Extend with Memory/Redb reopen parity. Use a controlled switch/barrier for ordering; the timeout is only the outer deadline.

**Separate-process restart pattern** (`apps/canary/src/publication.rs:344-388`, committed):

```rust
let mut child = spawn_crash_child(&database, &marker, &relay.url, seed, artifacts.root())?;
wait_child_marker(&marker, &mut child).await?;
child.kill().await?;
let status = child.wait().await?;
// reopen the same durable store through its supported constructor
let store = Arc::new(RedbWriteStore::open(&database).map_err(error)?);
let recovered = store.receipt(ReceiptId::from_u64(marker.receipt_id))?;
let fava = assembly(/* fresh runtime owners */, store, /* providers */)?;
let receipt = wait_terminal(&fava, ReceiptId::from_u64(marker.receipt_id)).await?;
```

For `ambiguous-handoff`, add the independent proxy witness that the complete EVENT crossed before the child/process is cut, then prove the same `Unknown` receipt after reopen. For `attempt-ceiling`, start offline, cross retry intervals without spent budget, then allow exact real failures to reach `GivenUp`.

**Build registration:** Cargo auto-discovers the integration tests, but Bazel does not. Add explicit `rust_test` targets in `crates/fava/BUILD.bazel`, following the adjacent declarations at lines 38-62 and 375-420, for `delivery_bounds`, `hostile_ingress`, and any new provider-isolation test.

---

### Hostile ingress and inbound wire-bound group (`HARD-03`, part of `HARD-08`)

**Current state:** dirty `WebSocketTransport`, its conformance test, and `fava/src/relay.rs`; untracked public corpus. Preserve and extend them. `crates/fava/src/relay.rs` is already 566 lines, so new process/harness behavior belongs outside this owner file; only owner admission/state logic belongs here.

**Primary analogs:** `apps/canary/src/hostile.rs`, `apps/canary/src/proxy.rs`, and `crates/fava/tests/multi_relay.rs`.

**Bound before parsing** (`crates/fava-transport-websocket/src/lib.rs:72-77`, dirty):

```rust
let config = WebSocketConfig::default()
    .max_message_size(Some(self.max_frame_bytes.get()))
    .max_frame_size(Some(self.max_frame_bytes.get()));
let (socket, _) = connect_async_with_config(key.relay.as_str(), Some(config), false)
    .await
    .map_err(|error| TransportError::ConnectionRefused(error.to_string()))?;
```

Keep the owner-level text-length check at `lib.rs:150-160` as defense in depth and exact diagnostic attribution. The WebSocket layer bounds allocation; the relay owner decides semantic admissibility.

**Terminal subscription identity belongs to the exact generation** (`crates/fava/src/relay.rs:67-73,166-233,327-333`, dirty):

```rust
terminated: BTreeSet<SubscriptionId>,

if self.terminated.contains(&id) {
    diagnostics.failed(key, generation, format!("EVENT after CLOSED for {id} is inert"));
    return;
}
let Some(filter) = self.attribution.get(&id) else {
    diagnostics.failed(key, generation, format!("unattributed EVENT for {id}"));
    return;
};
admit_subscription_event(/* exact session, id, filter, event */)?;
```

Clear `terminated` only when the same demand is deliberately reopened after auth or when a fresh transport generation replaces the session (`relay.rs:331-333,372-376`). Never compare relay URL alone.

**Adopt the existing public hostile corpus** (`crates/fava/tests/hostile_ingress.rs:194-226,253-293`, untracked):

```rust
hostile.push(&RelayMessage::event(hostile_id.clone(), forged));
hostile.push(&RelayMessage::event(hostile_id.clone(), off_filter));
hostile.push_raw("{not json at all");
hostile.push(&RelayMessage::event(SubscriptionId::new("never-requested"), event));
hostile.push(&RelayMessage::closed(hostile_id.clone(), "shutting down"));
hostile.push(&RelayMessage::event(hostile_id.clone(), after_closed));

// Then drive the healthy relay and assert exactly its event/evidence entered state.
assert_eq!(cache.len().expect("cache is readable"), 1);
```

This is a scripted owner/public test, not the required process proof. Extend its missing hostile classes without replacing its healthy-concurrent witness.

**Real WebSocket causal witness** (`apps/canary/src/hostile.rs:23-95`, committed partial analog):

```rust
let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
let server = tokio::spawn(async move {
    let (stream, _) = listener.accept().await?;
    let mut socket = accept_async(stream).await?;
    let request = next_text(&mut socket).await?;
    // derive the actual subscription id from Fava's REQ
    socket.send(Message::Text(hostile_frame.into())).await?;
    Ok::<_, CanaryError>(())
});
// Public Fava observes through WebSocketTransport; assert cache and public view stay empty.
```

For HARD-10, move the adversary from a Tokio task to a hidden child process using the `publication_child.rs:27-77` command/marker convention. The parent owns ports, scripts, gates, deadline, kill, and reap.

**Independent proxy transcript** (`apps/canary/src/proxy.rs:178-205`, committed):

```rust
message = downstream_stream.next() => {
    let message = message?;
    log.record(connection, "client_to_relay", &message)?;
    upstream_sink.send(message).await?;
}
message = upstream_stream.next() => {
    let message = message?;
    log.record(connection, "relay_to_client", &message)?;
    downstream_sink.send(message).await?;
}
payload = inject.recv() => {
    let message = Message::Text(payload.into());
    log.record(connection, "proxy_to_client", &message)?;
    downstream_sink.send(message).await?;
}
```

Extend this owner with explicit bounded gates for full-frame-crossed, drop-before-OK, truncation/disconnect, and no-wire assertions. Do not silently ignore `broadcast::RecvError::Lagged`; HARD-08 requires an exact loss/refusal fact.

---

### OPS-004 owner/resource ledger and bounds (`HARD-08`)

**Current state:** absent as an exhaustive ledger. Existing bounds are scattered. Create the ledger first, then modify only the owners whose rows prove a gap.

**No complete analog exists.** Use the following local patterns row-by-row.

**Typed refusal before custody/mutation** (`crates/fava/tests/write_bounds.rs:37-52,145-198`, committed):

```rust
assert_eq!(
    WriteIntent::event(event, WriteRouting::Explicit(relays)),
    Err(WriteIntentError::TooManyExplicitRelays {
        actual: 257,
        maximum: 256,
    })
);

let error = store.apply_route(/* 257 destinations */).unwrap_err();
assert_eq!(
    error.to_string(),
    "write store refused operation: route destination fan-out exceeds bound: 257 > 256"
);
assert_eq!(store.receipt(receipt_id)?.route_revision, 0);
```

Every ledger row should name: owner, resource/input, configured maximum, overflow behavior, exact typed refusal/backpressure/shortfall, current high-water/loss evidence, owner test, public test, and process-envelope field.

**Bounded retained categories** (`crates/fava-diagnostics/src/lib.rs:76-90,268-275`, committed but incomplete):

```rust
pub fn bounded(capacity: NonZeroUsize) -> Self {
    Self { capacity, state: Mutex::new(State::default()) }
}

fn push_bounded<T: Eq>(queue: &mut VecDeque<T>, capacity: usize, value: T) {
    if let Some(index) = queue.iter().position(|current| current == &value) {
        queue.remove(index);
    }
    if queue.len() == capacity {
        queue.pop_front();
    }
    queue.push_back(value);
}
```

Reuse the per-category `VecDeque`/oldest-first shape, but do not copy its silent eviction. HARD-08 requires an exact loss count or explicit refusal/backpressure fact. Extend `DiagnosticsSnapshot` and `relay_facts.rs:17-42` so exceeding capacity proves both retained facts and loss accounting.

**Latest-state backpressure** (`crates/fava/tests/observation_bounds.rs:47-77`, committed): canceled pulls plus a 256-event burst yield one latest snapshot, and `coalesced_query_updates > 0` reports suppressed intermediate revisions. Use this coalesced-current-state pattern only for observations where intermediate values are explicitly not promised.

**Artifact/resource starting point** (`apps/canary/src/artifacts.rs:21-36,64-78,89-98`, committed but incomplete):

```rust
File::create(root.join("resources.csv"))?
    .write_all(b"unix_ms,pid,rss_kib,generation\n")?;

pub(crate) fn record<T: Serialize>(&mut self, kind: &str, data: T) -> CanaryResult<()> {
    self.sequence = self.sequence.checked_add(1)
        .ok_or_else(|| CanaryError::new("evidence sequence exhausted"))?;
    // append one flushed JSONL fact
}

pub(crate) fn record_resource(&self, pid: u32, generation: u64) -> CanaryResult<()> {
    // independent `ps` witness, appended to resources.csv
}
```

This does not yet satisfy the envelope: it samples RSS only once, recursively hashes every file without count/byte bounds, and has no FD/task/queue/subscription/diagnostic/artifact high-water schema. Extend it with explicit sample cadence/deadline, maxima, artifact count/bytes, typed overflow, and a validated envelope per run.

---

### Provider failure isolation (`HARD-09`)

**Current state:** absent as a complete runtime behavior. Keep tests and implementation at existing provider owners; do not introduce an empty runtime framework.

**Closest partial analog:** `crates/fava-publication/src/materialization.rs`  
**Public failure corpus analog:** `crates/fava/tests/semantic_write_failures.rs` and `semantic_write_failures/reservation.rs`

**Panic-to-scoped-error conversion plus result validation** (`materialization.rs:218-235,432-456`, committed):

```rust
let invocation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    materializer.materialize(edit, *author, source, created_at)
}));
let event = match invocation {
    Ok(Ok(event)) => event,
    Ok(Err(error)) => return Err(PublicationError::Routing(
        format!("semantic materialization refused: {error}")
    )),
    Err(_) => return Err(PublicationError::Routing(
        "semantic materializer panicked".to_owned()
    )),
};
validate_materialization(edit, *author, &event, intent.routing(), created_at)?;
```

Copy the panic/refusal/malformed-result separation. Validation must compare exact accepted owner identity/generation/coordinate, not merely parse success.

**Release owner capacity after provider failure** (`semantic_write_failures/reservation.rs:12-38`, committed):

```rust
for failure in [ERROR, PANIC, WRONG_TIMESTAMP] {
    materializer.set(failure);
    assert!(fava.publish(edit_intent(/* ... */)).is_err());
    materializer.set(VALID);
    let accepted = fava.publish(edit_intent(/* ... */))
        .expect("the same sole capacity slot is reusable");
    assert!(fava.cancel_write(accepted.receipt_id)?);
}
```

Apply the same unrelated-progress assertion to query, relay, write, and shutdown paths.

**Missing pattern:** no current code provides the complete panic + blocking + late + malformed + cancellation-ignore boundary outside owner locks/store transactions with bounded shutdown. The planner must assign each provider call to its current owner, add causal barriers, and prove:

1. provider invocation begins after owner lock/transaction release;
2. timeout/cancel produces a scoped exact outcome;
3. a late completion carries exact operation/generation identity and is inert;
4. unrelated public work completes before the stalled provider is released;
5. shutdown joins, detaches with explicit evidence, or refuses within its declared bound.

Use `tokio::time::timeout` only as the outer ceiling; barriers/channels establish order. Any new public/cross-crate nominal boundary requires the separate vocabulary approval process.

---

### Seven M8 canaries, process harness, evidence envelopes (`HARD-01`-`HARD-10`)

**Current state:** all seven M8 executors, registry rows, and CLI dispatch arms are absent. Khatru exists but is not integrated. Authentication and NIP-11 owner logic are already committed; these files add capstone evidence, not replacement implementations.

**Primary scenario-module analog:** `apps/canary/src/semantic_writes.rs`  
**Real relay/restart analog:** `apps/canary/src/publication.rs` and `relay.rs`  
**Failure bundle analog:** `apps/canary/src/semantic_failure.rs`

**Private module dispatch and durable failure bundle** (`semantic_writes.rs:31-75`, committed):

```rust
pub(crate) fn has_executor(id: &str) -> bool {
    matches!(id, "scenario-a" | "scenario-b" /* ... */)
}

pub async fn run_semantic_write_scenario(
    id: &str,
    options: SmokeOptions,
) -> CanaryResult<PathBuf> {
    if !has_executor(id) {
        return Err(CanaryError::new(format!("unknown M7 scenario: {id}")));
    }
    let mut artifacts = RunArtifacts::create(&options.runs_directory, id, &options.seed)?;
    let outcome = match id { /* exact executor per id */ };
    match outcome {
        Ok(details) => finish(artifacts, id, &options, &details),
        Err(failure) => {
            let root = write_failure_bundle(artifacts, id, &options, &failure.to_string())?;
            Err(CanaryError::new(format!("{failure}; durable evidence: {}", root.display())))
        }
    }
}
```

Create the M8 module with the seven exact IDs:

- `nip42-write-and-reconnect`
- `auth-account-isolation`
- `hostile-relay-ingress`
- `relay-limit-shortfall`
- `ambiguous-handoff`
- `attempt-ceiling`
- `provider-failure-isolation`

**Registry-to-executor gate** (`apps/canary/src/lib.rs:137-175`, `lib_tests.rs:3-15`, committed): parse the embedded registry once, route `has_executor` to the private M8 module, and keep `every_enabled_scenario_has_an_executor`. Strengthen the test to require every M8 ID to be present exactly once, enabled, owned by M8, and dispatched by the CLI—not merely recognized by `has_executor`.

**CLI dispatch convention** (`apps/canary/src/main.rs:88-139`, committed):

```rust
let evidence = match scenario.as_str() {
    "existing-a" | "existing-b" => run_existing_scenario(/* ... */).await?,
    // one grouped arm for the seven M8 ids
    _ => return Err(std::io::Error::other(
        format!("unknown or unimplemented scenario: {scenario}")
    ).into()),
};
println!("passed {scenario}");
println!("evidence: {}", evidence.display());
```

Do not mark a registry/feature row `enabled`/`built` before registry, `has_executor`, CLI arm, exact executor, and evidence-schema test all exist.

**Bounded redacted failure artifact** (`apps/canary/src/semantic_failure.rs:11-65`, committed):

```rust
const FAILURE_DETAIL_CAPACITY: usize = 65_536;
let failure = bounded_failure(failure);
artifacts.write_json("failure.json", &json!({"scenario": id, "error": failure}))?;
artifacts.write_json("replay.json", &json!({
    "program": "cargo",
    "args": [/* exact replay, seed redacted */],
    "redacted_inputs": ["seed"],
    "scenario_seed_sha256": seed_hash(&options.seed),
}))?;
```

Copy into an M8/general failure utility rather than coupling M8 to semantic-write naming. Add the resource envelope and enforce total bundle count/bytes; the current 65,536-byte single-string cap is necessary but insufficient.

**Third-party relay process lifecycle** (`apps/canary/src/relay.rs:83-112,134-163,173-190`, committed): spawn with null stdin, file-owned stdout/stderr, `kill_on_drop`, exact PID/generation, readiness deadline, and bounded graceful-stop fallback to kill/reap. Extend configuration with an explicit authenticated profile; the existing config hardcodes `nip42_auth = false` at lines 222-224.

**Second relay fixture** (`apps/canary/relays/khatru/main.go:20-93`, committed): it already exposes loopback port, NIP-11 limits, optional auth, and enforced subscription count. Integrate it as a child process; do not rewrite its protocol behavior in Rust. Go 1.25 is a Wave 0 prerequisite before `go mod verify`, `go test ./...`, build, and scenario use. Its `SliceStore` is in-memory, so use persistent `nostr-rs-relay` for the NIP-42 restart/persistence witness and Khatru for the second implementation/core subset and advertised limits.

---

### Feature/evidence mapping

**Current state:** `features/relay-authentication.feature:5,20` and `features/relay-limits.feature:5` claim canaries that do not exist. Their owner/public Rust evidence is committed, but process evidence is not.

**Closest analog:** `tools/tests/test_semantic_write_feature.py`

**Fail-closed parser and real-target resolution** (`test_semantic_write_feature.py:24-68,71-83,237-243`, committed):

```python
def parse_feature(text: str):
    # attach exactly one well-formed mapping comment to one scenario
    # collect malformed, duplicate, and trailing mappings
    return scenarios, malformed_mappings, pending_mapping

def validate_mapping_target(target, listed_lines, expected_test):
    if target is None or "test" not in target.get("kind", []):
        raise ValueError("mapping target does not resolve to a Cargo test target")
    if listed_tests.count(expected_test) != 1:
        raise ValueError("mapped test must occur exactly once in Cargo --list output")

def test_every_mapping_resolves_to_one_real_cargo_test(self):
    for scenario in self.scenarios:
        self.assertEqual(cargo_mapping_evidence(scenario["mapping"]),
                         scenario["mapping"]["test"])
```

Add canary mapping resolution against `apps/canary/scenarios.json`, library `has_executor`, and CLI dispatch. Until an M8 process scenario passes, downgrade/remove only its nonexistent canary claim; keep the committed Rust evidence accurate. Restore `built` only when all named evidence resolves.

## Shared Patterns

### Public-Fava capstone

Owner/conformance tests establish causes; every HARD claim finishes through the public `Fava` facade. Existing assembly conventions are visible in `crates/fava/tests/delivery_bounds.rs:127-142`, `hostile_ingress.rs:170-183`, and `apps/canary/src/publication.rs:391-408`.

### Exact identity at every late boundary

Carry stable write/receipt/materialization/session/attempt/generation identity through request, provider invocation, completion, store transition, diagnostic, and evidence. Reject stale work before mutation. Redb's `begin_attempt`/`record_outcome` checks at `ops.rs:265-379` are the strongest current model.

### Independent external witness

Do not use Fava diagnostics to prove their own external effect. Use proxy JSONL, relay log/database, PID/port/process status, child marker, or independent resource sampler. `WireProxy::record` at `proxy.rs:124-142` flushes one sequenced direction-tagged frame fact.

### Controlled schedule, bounded deadline

Use channels, barriers, watch values, proxy gates, or child marker files to prove order. Timeouts are outer liveness ceilings. `publication.rs:344-388` and `publication_child.rs:27-77` are the process pattern; do not use sleep as the causal witness.

### Bounded error and artifact facts

Validate externally influenced text at its owning store/diagnostic boundary, preserve exact truncation/loss counts, and hash a bounded artifact set. Current `semantic_failure.rs` demonstrates redaction and a single detail cap; current `artifacts.rs` must be extended before it satisfies HARD-08/10.

### Contract/implementation parity

Neutral contracts own shared meaning; Memory and Redb must run the same lifecycle corpus. Do not let one provider define semantics privately. The dirty `spent_attempts` constructors and committed Redb `ops.rs` show where parity currently spans files.

### Vocabulary and file size

No new Rust package is needed. Any new public/cross-crate nominal value, provider contract, persisted entity, or lifecycle owner requires a separate approved architecture change and vocabulary checks. Prefer private canary/test modules. Do not grow `crates/fava/src/relay.rs` further with harness code; it is already above the 500-line soft limit.

## Already Completed — Do Not Re-plan

| Capability | Committed evidence | Remaining-only use |
|---|---|---|
| Generation-scoped NIP-42 owner and scripted public integration | `ed6a76c`; `crates/fava-auth`; `crates/fava/tests/authentication.rs` | real third-party relay, reconnect/persistence, two-account process evidence, approval checkpoint |
| Typed NIP-11 acquisition/projection and pre-wire scripted refusal | `94e04cd`; `fava-nip11{,-http}`; relay-limit tests | real advertised document, independent no-wire witness, Khatru second implementation |
| Focused unreachable retry/generation fix | `197c278`; committed Redb ops/test and public delivery test | reconcile dirty definitions/parity, durable reopen, process canaries |
| Khatru fixture source | `apps/canary/relays/khatru/main.go` | provision Go 1.25 and integrate; do not recreate |

## No Complete Analog Found

| File/Responsibility | Role | Data flow | Reason |
|---|---|---|---|
| `08-RESOURCE-LEDGER.md` plus validated resource-envelope schema | config/evidence | batch + process sampling | Existing bounds and `resources.csv` are scattered/partial; no exhaustive OPS-004 owner ledger exists. |
| Complete provider execution isolation boundary | service/runtime behavior | request-response, event-driven, cancellation | Current materializer path catches panic and validates output, but no general precedent covers blocking, late completion, ignored cancellation, unrelated progress, and bounded shutdown together. |
| Separate-process hostile relay/proxy script runner | process harness | streaming | Current `hostile.rs` uses an in-process Tokio task; `publication_child.rs` is the closest process analog but does not drive hostile wire scripts. |

## Metadata

**Analog search scope:** `crates/fava*`, `apps/canary`, `features`, `tools/tests`, Phase 07 summaries, current Git status/history  
**Strong analog files read:** `fava-write-store-redb/src/ops.rs`, `fava-publication/src/materialization.rs`, `apps/canary/src/{publication,publication_child,semantic_writes,semantic_failure,hostile,proxy,relay,artifacts,lib,main}.rs`, current dirty files/tests, feature mapping checker  
**Current source files preserved:** 12 modified tracked + 1 untracked  
**Pattern extraction date:** 2026-08-21
