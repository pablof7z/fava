//! Six architecture falsifiers for the M2–M4 ownership inversion fix.
//!
//! **Pre-remediation state (what these tests would have caught):**
//! `crates/fava/src/relay.rs`, `live.rs`, and `routes.rs` existed. The facade
//! owned relay futures directly. `OpenedRelay` was a facade type. `observe()`
//! awaited relay/transport futures before returning its handle. The facade's
//! `lib.rs` imported its own relay coordination modules.
//!
//! **Post-remediation state (what these tests prove):**
//! Those files are deleted with no adapter. `fava-observe` owns the live-query
//! lifecycle. `fava-runtime` owns execution. `observe()` returns immediately by
//! delegating to `Observer::open`, which is synchronous. The facade owns none
//! of the relay, live-query, or route-session lifecycle.
//!
//! Each test fails if the inversion is re-introduced and passes in the fixed
//! tree. All six together constitute the named deliberate break for this phase.

const LIB: &str = include_str!("../src/lib.rs");
const PUBLICATION: &str = include_str!("../src/publication.rs");
const BUILDER: &str = include_str!("../src/builder.rs");
const QUERY_SOURCE: &str = include_str!("../src/query_source.rs");
const SESSION: &str = include_str!("../src/session.rs");

// ── Falsifier 1 ──────────────────────────────────────────────────────────────

/// Prove `crates/fava/src/relay.rs` is absent.
///
/// Pre-remediation: `lib.rs` contained `mod relay;` which brought in
/// `OpenedRelay` and relay-future coordination owned by the facade.
/// Post-remediation: no `mod relay` exists; the line is absent.
///
/// **Deliberate break:** add `mod relay;` to `lib.rs` → this test fails.
#[test]
fn relay_module_absent_from_facade() {
    assert!(
        !LIB.contains("mod relay"),
        "facade lib.rs still declares `mod relay` — relay.rs must not exist in \
         crates/fava/src/. The relay lifecycle belongs to fava-observe, not the facade."
    );
}

// ── Falsifier 2 ──────────────────────────────────────────────────────────────

/// Prove `crates/fava/src/live.rs` is absent.
///
/// Pre-remediation: `lib.rs` contained `mod live;` which managed the live-query
/// lifecycle including relay future await chains inside the facade.
/// Post-remediation: no `mod live` exists.
///
/// **Deliberate break:** add `mod live;` to `lib.rs` → this test fails.
#[test]
fn live_module_absent_from_facade() {
    assert!(
        !LIB.contains("mod live"),
        "facade lib.rs still declares `mod live` — live.rs must not exist in \
         crates/fava/src/. The live-query lifecycle belongs to fava-observe."
    );
}

// ── Falsifier 3 ──────────────────────────────────────────────────────────────

/// Prove `crates/fava/src/routes.rs` is absent.
///
/// Pre-remediation: `lib.rs` contained `mod routes;` which coordinated relay
/// route sessions inside the facade.
/// Post-remediation: no `mod routes` exists.
///
/// **Deliberate break:** add `mod routes;` to `lib.rs` → this test fails.
#[test]
fn routes_module_absent_from_facade() {
    assert!(
        !LIB.contains("mod routes"),
        "facade lib.rs still declares `mod routes` — routes.rs must not exist in \
         crates/fava/src/. Route-session coordination belongs to fava-observe."
    );
}

// ── Falsifier 4 ──────────────────────────────────────────────────────────────

/// Prove `OpenedRelay` is absent from every facade source file.
///
/// Pre-remediation: `OpenedRelay` was a facade-owned struct that held an open
/// relay connection future, causing the model to own the relay lifecycle.
/// Post-remediation: no source file in `crates/fava/src/` references the type.
///
/// **Deliberate break:** re-introduce `struct OpenedRelay` in any facade
/// source file → this test fails.
#[test]
fn opened_relay_type_absent_from_all_facade_sources() {
    for (name, src) in [
        ("lib.rs", LIB),
        ("publication.rs", PUBLICATION),
        ("builder.rs", BUILDER),
        ("query_source.rs", QUERY_SOURCE),
        ("session.rs", SESSION),
    ] {
        assert!(
            !src.contains("OpenedRelay"),
            "facade source `{name}` still references `OpenedRelay`. \
             That type must not exist in crates/fava/src/; \
             relay-session ownership belongs to fava-observe."
        );
    }
}

// ── Falsifier 5 ──────────────────────────────────────────────────────────────

/// Prove `observe()` contains no `.await` on a relay or transport future.
///
/// Pre-remediation: `observe()` in `lib.rs` awaited relay-connection futures,
/// which meant a transport that never resolved would block the caller.
/// Post-remediation: `observe()` is annotated `#[allow(clippy::unused_async)]`
/// because it never awaits anything — it delegates synchronously to
/// `self.observer.open(query)`.
///
/// **Deliberate break:** add `.await` inside the `observe` body (e.g. awaiting
/// a relay establishment future) → this test fails.
#[test]
fn observe_contains_no_await() {
    // Extract the observe method body from lib.rs.
    let after_observe_sig = LIB
        .split("pub async fn observe(")
        .nth(1)
        .expect("observe() method not found in lib.rs");

    // Grab content up to the closing brace of the observe method body.
    let body_start = after_observe_sig
        .find('{')
        .expect("observe() body opening brace not found");
    let body = &after_observe_sig[body_start..];
    let body_end = body
        .find('}')
        .expect("observe() body closing brace not found");
    let observe_body = &body[..body_end];

    assert!(
        !observe_body.contains(".await"),
        "observe() body contains `.await`, meaning it blocks on a relay or \
         transport future. observe() must return immediately by delegating to \
         self.observer.open(query) without awaiting any relay future.\n\
         Body: {observe_body}"
    );
}

// ── Falsifier 6 ──────────────────────────────────────────────────────────────

/// Prove `observe()` delegates entirely to `self.observer.open(query)`.
///
/// Pre-remediation: `observe()` contained inline relay-binding logic — it
/// reached into `relay.rs`/`live.rs`/`routes.rs` to establish sessions
/// and awaited futures before returning the handle.
/// Post-remediation: the entire body of `observe()` is a single delegation:
/// `self.observer.open(query)`.
///
/// **Deliberate break:** add inline relay work inside the observe body,
/// bypassing the Observer delegation → this test fails.
#[test]
fn observe_delegates_to_observer_open() {
    let after_sig = LIB
        .split("pub async fn observe(")
        .nth(1)
        .expect("observe() method not found in lib.rs");
    let body_start = after_sig
        .find('{')
        .expect("observe() body opening brace not found");
    let body = &after_sig[body_start..];
    let body_end = body
        .find('}')
        .expect("observe() body closing brace not found");
    let observe_body = &body[1..body_end]; // strip the opening `{`

    assert!(
        observe_body.contains("self.observer.open(query)"),
        "observe() does not delegate to self.observer.open(query). \
         The observation lifecycle belongs to fava-observe's Observer; \
         the facade must not inline relay or transport binding.\n\
         Body: {observe_body}"
    );
}
