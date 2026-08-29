//! Scenarios whose only implementation was a workaround around Fava.
//!
//! Every entry here used to pass. Each one passed by doing something an
//! application cannot do: building a second engine to feed the first,
//! installing a stub transport or publisher so no relay was involved,
//! hand-feeding the library data it should have acquired, or driving Fava's
//! internal crates directly because the public facade had no door.
//!
//! The workaround has been deleted. The scenario is retained here so the wall
//! it was hiding is visible and so `canary run <id>` fails loudly instead of
//! reporting a green result that was never earned.

use crate::{CanaryError, CanaryResult};

/// One removed scenario and the Fava wall its workaround concealed.
pub struct Blocked {
    /// Scenario identifier as registered in `scenarios.json`.
    pub id: &'static str,
    /// The workaround that used to make it pass.
    pub workaround: &'static str,
    /// The public-API gap the workaround was hiding.
    pub wall: &'static str,
    /// Severity for an application that needs this behaviour.
    pub severity: &'static str,
}

/// Every scenario removed because it could only pass through a workaround.
pub const BLOCKED: &[Blocked] = &[
    Blocked {
        id: "async-recipient-routing",
        workaround: "built a second Fava with its own MemoryWriteStore and WebSocketTransport \
                     purely to hand OutboxRouter::new a QuerySource, then called \
                     OutboxRouter::remember with hand-built kind-10002 relay lists so the \
                     router never had to fetch them",
        wall: "OutboxRouter requires an Arc<dyn QuerySource> at construction, routers must be \
               handed to FavaBuilder before build, and the only realistic QuerySource is the \
               engine being built. The dependency is circular, so outbox routing cannot be \
               assembled by an application at all without a second engine. WRITE-014 forbids \
               the separate transport stack that the second engine creates.",
        severity: "show-stopper",
    },
    Blocked {
        id: "hint-routing",
        workaround: "called HintRouter::remember with an EventRecord the canary pulled out of \
                     an observation, hand-feeding the router the relay evidence Fava had \
                     already ingested",
        wall: "Fava never offers its own ingested relay evidence to its own routers. An \
               application must observe, project records itself, and push them back into each \
               router on every update. Nothing in the public API says this is required, and \
               nothing does it for you.",
        severity: "major",
    },
    Blocked {
        id: "route-preview-parity",
        workaround: "called fava_routing::preview directly, reaching into an internal crate for \
                     a write-route preview",
        wall: "Fava::preview_routes takes a &Query and only previews read routes. There is no \
               public preview for a write, so an application cannot show a user where a post \
               will go before accepting it.",
        severity: "major",
    },
    Blocked {
        id: "app-relay-versus-fallback-profile",
        workaround: "same internal fava_routing::preview call to compare two write profiles",
        wall: "Same missing public write-route preview as route-preview-parity.",
        severity: "major",
    },
    Blocked {
        id: "replaceable-edit-first-value",
        workaround: "installed a NoopTransport that refuses every connection and a canary-owned \
                     Publisher that returns Acknowledged without sending anything, then \
                     advertised the run as a public Fava execution against a relay",
        wall: "Fava offers no way to observe a publication lifecycle deterministically without \
               a relay, and no in-process or loopback transport provider ships with it. The \
               only deterministic option is to implement fava-transport and fava-publisher \
               yourself, at which point no Fava publication path is under test.",
        severity: "major",
    },
    Blocked {
        id: "replaceable-edit-reapplication",
        workaround: "NoopTransport, a gate Publisher that holds and releases attempts, and a \
                     CompletionStore that reimplements all twenty-three WriteStore methods as \
                     forwarding wrappers over MemoryWriteStore purely to observe when signing \
                     completed",
        wall: "There is no public signal for publication lifecycle transitions. \
               Fava::receipt_changes reports committed receipts but not revision \
               installation, signer refusal, route application, or attempt start. To see them \
               an application must implement the internal WriteStore contract in full.",
        severity: "major",
    },
    Blocked {
        id: "replaceable-edit-opposing-operations",
        workaround: "NoopTransport plus the canary Publisher and CompletionStore",
        wall: "Same as replaceable-edit-reapplication.",
        severity: "major",
    },
    Blocked {
        id: "protocol-crate-n-plus-one",
        workaround: "NoopTransport plus the canary Publisher; protocol semantics are exercised \
                     without claiming fabricated live relay provenance",
        wall: "Fava still has no public import door that can establish verified live provenance; \
               RelayOccurrence is intentionally produced only by attributed admission.",
        severity: "major",
    },
    Blocked {
        id: "subscription-grouping-equivalence",
        workaround: "drove SubscriptionPlanner, its own Transport, fava_wire encode/decode and \
                     fava_ingest::admit_subscription_event by hand, bypassing Fava::observe \
                     entirely, and then wrote result_equivalence: true and \
                     relay_source_evidence_equivalence: true into the retained manifest as \
                     literals rather than as measured values",
        wall: "Fava opens one WebSocket and one REQ per Observation and never groups \
               subscriptions across observations (see flow-07-shared-connection). There is \
               therefore no way to exercise a grouping planner through the public API, and no \
               way to observe how many REQs an application's queries cost. The equivalence the \
               manifest claimed was never computed from the run.",
        severity: "major",
    },
    Blocked {
        id: "local-source-merge",
        workaround: "called the concrete local providers directly and therefore could not prove \
                     a public local-state import door",
        wall: "Fava has no public door for putting an event into local state. \
               Fava::accept_event is documented as a compile-fail. An application therefore \
               cannot import a backup, restore a cache, seed a first run, or write a local-only \
               draft without holding the concrete provider and calling the internal EventCache \
               or WriteStore contract, and without fabricating relay provenance.",
        severity: "show-stopper",
    },
    Blocked {
        id: "local-replaceable-shadow-and-cancel",
        workaround: "same hand-fed EventCache::admit and WriteStore::accept_applied calls",
        wall: "Same missing local-state door as local-source-merge.",
        severity: "show-stopper",
    },
    Blocked {
        id: "local-source-removal",
        workaround: "same hand-fed EventCache::admit, plus EventCache::expire to drive retraction",
        wall: "Same missing local-state door as local-source-merge. Cache expiry is also \
               reachable only through the internal contract, so an application cannot age out \
               its own storage.",
        severity: "show-stopper",
    },
    Blocked {
        id: "slow-consumer-latest-state",
        workaround: "called WriteStore::accept_applied 256 times to produce a burst",
        wall: "Same missing local-state door as local-source-merge.",
        severity: "show-stopper",
    },
];

/// Look up a removed scenario.
#[must_use]
pub fn blocked(id: &str) -> Option<&'static Blocked> {
    BLOCKED.iter().find(|entry| entry.id == id)
}

/// Fail with the exact wall a removed scenario's workaround was concealing.
///
/// # Errors
///
/// Always returns an error. That is the point.
pub fn refuse(entry: &Blocked) -> CanaryResult<()> {
    Err(CanaryError::new(format!(
        "scenario {} is blocked on a Fava wall [{}]\n  removed workaround: {}\n  wall: {}",
        entry.id, entry.severity, entry.workaround, entry.wall
    )))
}
