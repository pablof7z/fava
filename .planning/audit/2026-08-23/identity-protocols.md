# Identity and protocol crates audit

Area slug: `identity-protocols`
Scope crates: `fava-signer`, `fava-signer-local`, `fava-nip02`, `fava-simple-groups`, `fava-bookmarks`
(plus the consumer edges those crates' contracts are exercised through: `fava-publication`,
`fava-publisher-nip01`, `fava/src/relay.rs`, `fava-write/src/receipt.rs`).

Mode: read-only. No production, test, or spec file was modified.

## Scope checked

Specs read (authority):

- `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` — GOAL-007/008/009 (218-262),
  WRITE-007..011 (799-840), WRITE-018/019 (905-930), RELAY-007/008 (1091-1105),
  ID-001..008 (1181-1235), PROTO-001..004 (1240-1290), protocol/service inventory (1350-1380),
  OPS-001 (1390-1400)
- `docs/spec/ARCHITECTURE.md` — replaceable-event edits (728-757), `fava-signer` (1766-1808),
  event-kind protocol crates / `fava-nip02` / `fava-simple-groups` (1862-2027),
  `fava-publication` (2114-2202), `fava-session` incl. runtime signer contract (2204-2284),
  `fava-auth` (2286-2303), single-owner map (2961-2995), crate responsibility tables (3595-3660)
- `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` — 6.1/6.2 causes-not-conclusions (205-230),
  §11 provider-contract TDD (355-372), §15 anti-patterns (475-495)
- `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` — M5 (560-660), M7 exit behavior (740-760),
  diagnostics minimums (1195-1215), mutation expectations (1230-1250)
- `docs/internals/vocabulary.toml` — `Signer` (757-776), `Session` (778-790), `Authentication`
  (835-844), `RelayAccess` (368-378), `BookmarkList` (180-190), `Group` (191-215),
  `IntoContactAuthors` (165-178)
- `AGENTS.md` gates and vocabulary policy

Code read in full:

- `crates/fava-signer/src/lib.rs`, `crates/fava-signer-local/src/lib.rs` (+ both `Cargo.toml`)
- `crates/fava-nip02/src/{lib,query,edit,bounds}.rs`, `tests/{public_api,architecture}.rs`,
  `src/tests/edit.rs` (targeted), `README.md`
- `crates/fava-simple-groups/src/{lib,query,group,management,edit,snapshot,bounds}.rs`,
  `tests/{public_api,architecture}.rs`
- `crates/fava-bookmarks/src/{lib,bounds}.rs`, `tests/public_api.rs`
- `crates/fava-publication/src/{lib,run,delivery}.rs`
- `crates/fava-publisher-nip01/src/lib.rs`, `crates/fava-write/src/receipt.rs`,
  `crates/fava-write-store/src/lib.rs` (signer entry points),
  `crates/fava-write-store-memory/src/lifecycle.rs`
- `crates/fava/src/{lib,relay,publication}.rs` (signer/auth surfaces),
  `crates/fava-query/src/lib.rs` (RelayAccess/acquisition), `crates/fava-state/src/lib.rs`,
  `crates/fava-diagnostics/src/lib.rs`
- `crates/fava/tests/simple_groups.rs` (whole file incl. harness)

Searches actually run (used to support the absence claims below):

- `grep -rn -i "nip44|nip04|decrypt" crates/` → **0 hits anywhere in the workspace**
- `grep -rn -i "relayaccess|nip.42|challenge|::Auth|Auth(" crates/` → only
  `RelayAccess::public()` call sites, `fava/src/relay.rs:300`, `fava-publisher-nip01/src/lib.rs:86`,
  `fava-publisher/src/lib.rs:44`, `fava-publication/src/delivery.rs:202`,
  `fava-diagnostics/src/lib.rs:35/58/174`
- `grep -rn "RelayAccess::" crates/ | grep -v "public()"` → **0 hits**
- `grep -rn "Unavailable" --include=*.rs crates/ apps/` → `SignerAvailability::Unavailable`
  is **never constructed** anywhere in `crates/` or `apps/`
- `grep -rn "timeout|Duration::from|sleep" crates/fava-publication/src crates/fava/src`
  → only `ATTEMPT_TIMEOUT` (delivery), `STORE_READ_RETRY_DELAY`, and a 50 ms relay poll
- `ls crates | grep testkit` → only `fava-router-testkit`, `fava-transport-testkit`
- `grep -rEn "^ *(pub\(crate\) |pub\(super\) )?(struct|enum|trait) " ` over all five scope crates
  → enumerated below under Conforming

## Findings

### nip42-auth-has-no-owner — critical — ownership

**authority**
`docs/spec/ARCHITECTURE.md:2981` — `| NIP-42 challenge lifecycle | fava-auth | query/publication owners |`
`docs/spec/ARCHITECTURE.md:2288-2300` — `fava-auth` **Responsibility:** own NIP-42 challenge and
authorization lifecycles for exact relay access. Owned state: "relay-access identity; current relay
challenge; application authentication-policy operation; signer operation for the AUTH event; current
session generation; accepted/refused/failed authentication facts; re-authentication after reconnect;
exact attribution of authentication outcomes to query and publication work."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1101` — "The application supplies an auth
policy for exact relay access. Fava answers challenges, supports challenge timing before or after a
request, and re-authenticates after reconnect."

**implementation**
`crates/fava/src/relay.rs:300` —
`RelayMessage::Auth { .. } => diagnostics.authentication_required(key, generation),`
The challenge string is destructured away with `..`; nothing else in the workspace ever reads an
AUTH challenge.
`crates/fava-publisher-nip01/src/lib.rs:85-88` — the publish loop likewise discards the challenge and
returns `PublishOutcome::AuthenticationRequired`.
`crates/fava-query/src/lib.rs:111` — every `Query` hardcodes `access: RelayAccess::public()`; there is
no builder method to name a relay access, `RelayAccess` is not re-exported by the `fava` facade
(`grep -rn "RelayAccess" crates/fava/src/` → 0 hits), and `RelayAccess::named` has zero call sites.
There is no auth-policy contract, no AUTH-event signing path, no re-auth on reconnect, and
`crates/fava-diagnostics/src/lib.rs:35` exposes only `authentication_required: Vec<SessionFact>`
(relay + generation) — none of the accepted/refused/failed facts `fava-auth` is required to own.

**observable distinction**
Point a `Fava` at a relay that answers `REQ` with `AUTH` (a common paid/private relay). The
observation stays permanently empty, the only trace is a `authentication_required` diagnostic tuple,
and no public API exists to supply a policy, sign the challenge, or attribute the shortfall. On a
public relay the same query works. RELAY-007's "challenge timing before or after a request" and
"re-authenticates after reconnect" have no code path at all.

**proposed falsifier**
```rust
#[tokio::test]
async fn auth_required_relay_serves_the_query_after_the_policy_signs_the_challenge() {
    let relay = scripted_relay().requires_auth("chal-1");           // sends AUTH, then REQ->AUTH-required
    let fava = Fava::builder()./* … */.auth_policy(Arc::new(SignWithAlice)).build()?;
    let mut obs = fava.observe(Query::events().from_relays([relay.url()])?).await?;
    relay.expect_client_auth_event_for("chal-1");                    // fails today: never sent
    assert_eq!(wait_for(&mut obs).events.len(), 1);
}
```

**confidence** confirmed

---

### auth-denied-collapsed-into-givenup — critical — behavioral proof

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:905-918` — "For each selected destination,
the receipt MUST preserve exact observable outcomes such as: … acknowledged, including the relay's
message; rejected, including the relay's message; **authentication denied**; backing off; given up; …"
`…:1103` — "that destination terminates with an **auth-denied outcome** while unrelated accounts and
destinations continue independently."
`…:218-224` (GOAL-007) — a provider "cannot construct stronger evidence or success than the facts
supplied to it justify", explicitly including "whether bytes may have left Fava".

**implementation**
`crates/fava-write/src/receipt.rs:23-55` — `RelayDeliveryOutcome` has
`Pending | Attempting | Retryable | Acknowledged | Rejected | GivenUp | Unknown | CancelledBeforeHandoff`.
There is **no** `AuthenticationDenied` variant.
`crates/fava-publication/src/delivery.rs:202-204` —
```rust
PublishOutcome::AuthenticationRequired => RelayDeliveryOutcome::GivenUp {
    reason: "relay authentication required".to_owned(),
},
```
`crates/fava-write/src/receipt.rs:43` documents `GivenUp` as "Bounded policy stopped after definite
**pre-handoff** failure" — yet `crates/fava-publisher-nip01/src/lib.rs:47-57` only reaches the AUTH
arm *after* `HandoffOutcome::HandedOff`, i.e. after the `EVENT` frame definitely left Fava.

**observable distinction**
Publish to an auth-requiring relay. The receipt reports a *pre-handoff* give-up for a destination
that provably received the bytes, with an invented policy reason, and the application cannot
distinguish "relay wants NIP-42" from "delivery policy exhausted its retries". The delivery policy
sees the same collapsed fact, so it cannot treat auth denial differently from give-up.

**proposed falsifier**
```rust
#[tokio::test]
async fn auth_required_destination_is_a_distinct_post_handoff_outcome() {
    let relay = scripted_relay().replies_auth_to_event();
    let write = fava.to([relay.url()])?.publish(signed_event())?;
    let receipt = write.settled(fava::all()).await?;
    assert!(matches!(receipt.destination(relay.key()), RelayDeliveryOutcome::AuthenticationDenied { .. }));
    assert_ne!(receipt.destination(relay.key()), &RelayDeliveryOutcome::GivenUp { .. }); // fails today
}
```

**confidence** confirmed

---

### signer-no-deadline-no-timed-out — critical — boundedness

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:811` (WRITE-007) — "Unavailable, rejected,
invalid-output, cancelled, **timed-out**, and stale signer results remain distinct."
`…:228` (GOAL-008) — providers "MUST NOT indefinitely block unrelated relays, queries, writes,
signers, or shutdown."
Contrast: WRITE-008 (`…:817`) forbids elapsed-time abandonment only for a write that *has no
available signer* — an in-flight provider operation is a different fact.

**implementation**
`crates/fava-signer/src/lib.rs:34-49` — `SignerError` has exactly four variants
(`Unavailable`, `Rejected`, `Cancelled`, `InvalidOutput`). No timed-out and no stale outcome exists
in the contract, so no provider or owner can express either.
`crates/fava-publication/src/run.rs:437-438` —
```rust
tokio::spawn(async move {
    match signer.sign_event(unsigned, cancel).await {
```
a bare await with no Fava-owned deadline. Compare `crates/fava-publication/src/delivery.rs:14`
(`const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);`) which *does* bound the publisher
provider call — signing is the only long-running provider call in the write lifecycle with no ceiling.

**observable distinction**
Supply a signer whose future never resolves (a plausible NIP-46 remote signer with a dead relay).
The receipt stays at `SignatureState::Unsigned` forever, no destination ever leaves `Pending`,
`write.settled(all())` never returns, no diagnostic fact is produced, and shutdown has an
unjoined task holding the provider. The identical relay-side hazard resolves as
`Unknown { reason: "publication deadline elapsed after handoff" }` in 5 s.

**proposed falsifier**
```rust
#[tokio::test(start_paused = true)]
async fn hung_signer_produces_a_timed_out_signer_fact_within_a_fava_owned_deadline() {
    let fava = harness_with_signer(Arc::new(NeverResolvingSigner::new(alice)));
    let write = fava.publish(unsigned_from(alice))?;
    tokio::time::advance(SIGNING_DEADLINE + Duration::from_secs(1)).await;
    let receipt = fava.receipt(write.receipt_id())?.unwrap();
    assert!(matches!(receipt.signature(), SignatureState::Refused(r) if r.contains("timed out")));
}
```

**confidence** confirmed

---

### publication-owns-signer-registry — critical — ownership

**authority**
`docs/spec/ARCHITECTURE.md:2982` — `| Signer registration and availability | fava-session plus signer
provider | publication/auth owners |`
`docs/spec/ARCHITECTURE.md:2261-2263` — "The public `Fava` facade delegates runtime `add_signer`,
explicit `replace_signer`, and `remove_signer` operations to this owner. Builder-supplied signers seed
the same `Session`; **they are not copied into publication-owned state.**"
`docs/spec/ARCHITECTURE.md:2254-2258` — "The returned signer generation identifies one exact
attachment. Publication must re-check that generation before installing a signer completion."
`docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:633` (M5 exit gate) — "`fava-publication` owns the
write lifecycle but **not** router, **signer**, publisher, transport, or delivery policy state."

**implementation**
`crates/fava-publication/src/lib.rs:33` — `signers: Arc<BTreeMap<PublicKey, Arc<dyn Signer>>>`,
built once in `Publication::new` (`lib.rs:53-72`) from the builder list and never mutated.
`crates/fava/src/lib.rs:259/335/345/423` — signers are builder-only; there is no
`Fava::add_signer`/`replace_signer`/`remove_signer` (`grep -n "add_signer" crates/fava/src` → 0 hits).
`crates/fava-publication/src/run.rs:426` — `self.signers.get(&unsigned.pubkey)` reads that private
copy. No attachment generation exists anywhere; `crates/fava-signer/src/lib.rs` defines no generation
or attachment type for anyone to own.

**observable distinction**
Accept an unsigned Alice event with no Alice signer attached; attach Alice's signer afterwards. The
vocabulary counterexample for `Session` (`docs/internals/vocabulary.toml:785`) says the same write
must begin signing in the same running `Fava`. It cannot: the only way to attach a signer is to build
a new `Fava`, which discards every in-flight publication lifecycle. ID-001's separation of session
state from accepted-write state is unobservable because there is no session state.

**proposed falsifier**
```rust
#[tokio::test]
async fn runtime_attached_signer_wakes_a_parked_write() {
    let fava = harness_with_signers([]);                 // no signer for alice
    let write = fava.publish(unsigned_from(alice))?;      // parked, per WRITE-008
    fava.add_signer(Arc::new(LocalSigner::new(alice_keys)))?;  // does not compile today
    let receipt = write.settled(fava::all()).await?;
    assert!(matches!(receipt.signature(), SignatureState::Signed));
}
```

**confidence** confirmed (overlaps the `fava-session`-absence auditor; reported here as the
signer-side ownership fact and the missing generation in the signer contract)

---

### stale-signer-completion-is-silent — major — failure isolation

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:809-811` — a signer completion is accepted
only if it "belongs to the current signer/provider operation", and "Unavailable, rejected,
invalid-output, cancelled, timed-out, and **stale** signer results remain distinct."
`…:230` (GOAL-008) — "Late completions MUST carry enough identity to be dropped when stale."
`docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:746` — "Signer, route, and delivery completions for
retired generations are rejected as stale."
`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:216` — a required distinction: "current request completion
versus stale previous-generation completion".

**implementation**
`crates/fava-signer/src/lib.rs:27-31` — `sign_event` returns `Result<Event, SignerError>`; the
completion carries no operation, attachment, or generation identity, so staleness can only be
inferred by the caller.
`crates/fava-publication/src/run.rs:440-465` — both store calls are discarded:
```rust
if publication.store.install_signed(write_id, receipt_id, materialization_id, event_id, event).is_err() {
    let _ = publication.store.record_signer_refusal(
        write_id, receipt_id, materialization_id, event_id,
        "signer returned an event that did not match the accepted body".to_owned());
}
```
`crates/fava-write-store-memory/src/lifecycle.rs:33/71` — `validate_current_materialization` rejects
*both* calls for a retired generation, so a late completion produces **no fact at all**. Worse, the
hard-coded reason string is applied to every `install_signed` failure regardless of cause, discarding
the store's actual `WriteStoreError` text (e.g. "event is already signed differently").

**observable distinction**
Force a rematerialization while generation 1 is signing, then let a provider that ignores the `cancel`
receiver answer for generation 1. From the public API this is indistinguishable from a signer that
never answered: no receipt change, no diagnostic, no counter. The mutation named at
`FAVA_REWRITE_IMPLEMENTATION_PLAN.md:1239` ("allow stale signer completion after rematerialization")
cannot be detected by any current test, because the correct behavior is also silent.

**proposed falsifier**
```rust
#[tokio::test]
async fn late_signer_completion_for_a_retired_materialization_is_an_attributable_stale_fact() {
    let signer = Arc::new(GatedSigner::ignoring_cancel(alice));   // answers generation 1 late
    let (fava, diag) = harness_with_signer(signer.clone());
    let write = fava.publish(edit_from(alice))?;
    force_rematerialization(&fava, &write);            // generation 2 becomes current
    signer.release_generation_one();
    assert_eq!(diag.snapshot().stale_signer_completions.len(), 1);   // zero today
}
```

**confidence** confirmed

---

### signer-availability-has-no-lifecycle — major — failure isolation

**authority**
`docs/internals/vocabulary.toml:762` — "Nostr defines signature verification; Signer defines the
application-owned signing operation and **availability lifecycle**."
`docs/spec/ARCHITECTURE.md:1796` — "Signer instances are attached to accounts at runtime because
login, hardware availability, remote signer connectivity, and human approval are application
lifecycles."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:817-821` (WRITE-008) — an accepted event
with "no available signer for its pubkey … remains awaiting that signer", and a fresh signer request
follows "when the correct provider becomes available".
`docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:1206` — required diagnostic dimension:
"signer/provider availability".

**implementation**
`crates/fava-signer/src/lib.rs:24` — `fn availability(&self) -> SignerAvailability;` is a
point-in-time poll. The trait offers no subscription, no change notification, and no way for a
provider to report that it became available. There is nothing in `SignerAvailability` beyond the two
states (`lib.rs:10-18`).
`crates/fava-publication/src/run.rs:429-431` —
```rust
if !matches!(signer.availability(), SignerAvailability::Available) {
    return;
}
```
The materialization is abandoned silently and nothing ever re-polls. `crates/fava-diagnostics/src/lib.rs:17-40`
exposes no signer/provider availability field at all.
`grep -rn "Unavailable" --include='*.rs' crates/ apps/` shows `SignerAvailability::Unavailable`
is never constructed in the workspace — the branch has zero evidence of any kind.

**observable distinction**
Attach a signer for Alice that reports `Unavailable` (a disconnected NIP-46 provider), accept an
Alice write, then have the provider reconnect and report `Available`. The write never signs, no
cancellation occurred, and no diagnostic reports why. A polling application cannot even observe the
provider's availability through Fava.

**proposed falsifier**
```rust
#[tokio::test]
async fn signer_that_becomes_available_signs_the_parked_write() {
    let signer = Arc::new(FlippableSigner::unavailable(alice));
    let fava = harness_with_signer(signer.clone());
    let write = fava.publish(unsigned_from(alice))?;
    signer.become_available();                       // no wakeup path exists today
    let receipt = write.settled(fava::all()).await?;
    assert!(matches!(receipt.signature(), SignatureState::Signed));
}
```

**confidence** confirmed

---

### signer-contract-omits-decrypt — major — replaceability

**authority**
`docs/spec/ARCHITECTURE.md:1772-1789` — the `Signer` contract is specified as
`public_key` / `availability` / `sign_event` / **`fn decrypt(&self, request: DecryptRequest) -> DecryptFuture;`**
`docs/spec/ARCHITECTURE.md:1792` — "Encryption and decryption use distinct request and outcome values
even when one provider implements both."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1221-1225` (ID-007) — "A provider may
implement one or several cryptographic operations, but the contracts and outcomes remain distinct.
NIP-44 and legacy NIP-04 support, where selected, MUST preserve exact account, source, operation, and
request identity. Unavailable, rejected, invalid ciphertext, malformed plaintext, cancellation, and
stale completion remain distinct."
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1362-1364` — NIP-44 encryption/decryption
and legacy NIP-04 decryption are named supporting services.

**implementation**
`crates/fava-signer/src/lib.rs:20-32` — the trait has only `public_key`, `availability`, `sign_event`.
There is no `DecryptRequest`, no decrypt outcome type, and no encryption surface.
`grep -rn -i "nip44|nip04|decrypt" crates/` returns **zero hits across the entire workspace**, so no
other crate compensates.

**observable distinction**
An application with a hardware or NIP-46 provider that can decrypt has no Fava contract to expose
that capability through, and no Fava crate can consume it. Any protocol crate needing private content
(private bookmarks — `docs/internals/vocabulary.toml:184` defines `BookmarkList` as "public **or
private** bookmarks"; NIP-29 private group content; gift wraps) is unimplementable without inventing
a parallel contract, which ID-007 forbids.

**proposed falsifier**
```rust
#[test]
fn decrypt_is_a_distinct_provider_operation_with_its_own_outcomes() {
    fn takes(_: &dyn fava_signer::Signer) {}
    let _: fn(&dyn Signer, DecryptRequest, watch::Receiver<bool>) -> DecryptFuture = Signer::decrypt;
    // does not compile today
}
```

**confidence** confirmed

---

### no-signer-conformance-kit — major — behavioral proof

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:236-248` (GOAL-009) — "Each replaceable
contract MUST ship a public conformance kit covering: ordinary behavior; refusal and malformed input;
cancellation and close; late completion; boundedness and overload; … The test kit MUST work from
public APIs."
`docs/spec/ARCHITECTURE.md:3648-3658` — recommended public test tooling includes `fava-signer-testkit`.
`docs/internals/vocabulary.toml:772-775` — `Signer`'s `spec_crates` list includes `fava-signer-testkit`.
`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:369` — "A trait with one implementation is not evidence of
substitutability."

**implementation**
`ls crates/fava-signer crates/fava-signer-local` — neither crate has a `tests/` directory; neither
`src/lib.rs` contains a `#[cfg(test)] mod tests`. The two crates ship **zero** tests between them.
`ls crates | grep testkit` → only `fava-router-testkit` and `fava-transport-testkit`; there is no
`fava-signer-testkit`. The only signer behavior anywhere is ad-hoc spy signers inside `fava` tests
(`crates/fava/tests/simple_groups.rs:630-707`) and `apps/canary/src/semantic_write_support.rs`, none
of which is reusable by an external provider crate.

**observable distinction**
An application shipping its own hardware/extension/NIP-46 signer has no public suite to run and no
way to demonstrate contract conformance; the specific cases GOAL-009 names (cancellation, **late
completion**, boundedness) are exactly the ones the two findings above show are unspecified.

**proposed falsifier**
```rust
// crates/fava-signer-testkit/src/lib.rs
pub async fn conformance(build: impl Fn() -> Arc<dyn Signer>) { /* ordinary, refusal, cancel, late, bounds */ }
// crates/fava-signer-local/tests/conformance.rs
#[tokio::test] async fn local_signer_conforms() {
    fava_signer_testkit::conformance(|| Arc::new(LocalSigner::new(Keys::generate()))).await;
}
```

**confidence** confirmed

---

### nip02-accepts-bech32-target — major — dependency direction / boundary refusal

**authority**
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1203-1205` (ID-004) — "Internal boundaries
use raw protocol identity values, not human-facing bech32 text. An application may decode `npub`,
`nprofile`, or other presentation forms at its input boundary. Fava MUST refuse the wrong identity
shape rather than silently reinterpret it where a raw pubkey is required."

**implementation**
`crates/fava-nip02/src/edit.rs:225-240` — `parse_target` renders `impl fmt::Display` into a bounded
string and calls `PublicKey::parse(&text.value)` (line 238), which in `nostr` accepts hex **and**
bech32 `npub1…` **and** `nostr:npub1…` NIP-21 URIs.
`crates/fava-nip02/src/edit.rs:17` — `const MAX_TARGET_TEXT_BYTES: usize = 69;` — sized above the
64-byte hex form specifically so bech32 fits.
`crates/fava-nip02/src/tests/edit.rs:17-28` asserts the deviation as intended behavior:
`assert_eq!(follow(target).expect("key"), follow(npub).expect("npub"));`
`crates/fava-nip02/README.md:181` contradicts the code: "Targets accept `PublicKey`, hex strings, and
owned hex strings."

**observable distinction**
`fava_nip02::follow("npub1…")` returns `Ok(ReplaceableEventEdit)` identical to the raw-key edit,
instead of `Err(WriteIntentError::InvalidEvent)`. A presentation-form string that the application was
supposed to decode at its own boundary is silently reinterpreted inside Fava, and the same acceptance
extends to `nostr:` URIs and any `Display` type whose rendering happens to parse.

**proposed falsifier**
```rust
#[test]
fn follow_refuses_bech32_presentation_text() {
    let npub = alice.to_bech32().unwrap();
    assert!(matches!(fava_nip02::follow(npub.as_str()), Err(WriteIntentError::InvalidEvent(_))));
    assert!(matches!(fava_nip02::follow(format!("nostr:{npub}")), Err(WriteIntentError::InvalidEvent(_))));
}
```

**confidence** confirmed

---

### group-relay-acquisition-unproven — major — behavioral proof

**authority**
`docs/spec/ARCHITECTURE.md:1983-1985` — "content queries add exactly one `h = group-id` constraint and
use `from_relays(hosts)`, preserving accepted local-write visibility **while asking exactly the
selected hosts**"
`docs/spec/ARCHITECTURE.md:1988-1989` — "the same event id appears once across hosts with every
**actual relay evidence** contribution"
`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:211` — a required distinction: "relay selected versus relay
**actually contacted**"
`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:487` — anti-pattern: "a line/count/grep gate presented as
behavioral proof"

**implementation**
`crates/fava/tests/simple_groups.rs` is the only facade-level group evidence. Every one of its seven
group queries is `.cache_only()` (lines 50, 86, 139, 204, 247, 310, 457), which sets
`Freshness::CacheOnly` (`crates/fava-query/src/lib.rs:156-159`) and therefore creates **no relay
demand at all**, while leaving `QueryAcquisition::Explicit(hosts)` in the query identity unexercised.
The harness transport is `SpyTransport` (`crates/fava/tests/simple_groups.rs:768-786`) whose
`open_session` always returns `Err(TransportError::ConnectionRefused("spy transport must remain
unopened"))`. Per-host provenance is hand-committed into the memory cache
(`cache.commit(vec![CacheMutation::Upsert(CachedEvent::new(event, evidence(relay, at)))])`, around
line 145) — the exact "set up conclusions, not causes" pattern `FAVA_TDD_BDD_TESTING_GUIDE.md:220`
rejects.
`crates/fava-simple-groups/tests/architecture.rs:181-334` is source-text `grep` over
`crates/*/src/**.rs`; `crates/fava-simple-groups/tests/public_api.rs:229-236`
(`readme_facade_flow_compiles_externally`) only coerces function pointers and never runs the flow.

**observable distinction**
Because `Fava::observe` is confirmed defective on the relay path (facade-owned session establishment,
blocking handle, leaked partial-open session), a real two-host group read may ask the wrong hosts,
open duplicate sessions, or block — and no test in the workspace would fail. The two claims that make
`Group` more than a tag helper — "asking exactly the selected hosts" and "every actual relay evidence
contribution" — have no falsifier through the real acquisition path.

**proposed falsifier**
```rust
#[tokio::test]
async fn group_content_query_asks_exactly_the_selected_hosts_over_real_sessions() {
    let (a, b, decoy) = (scripted_relay(), scripted_relay(), scripted_relay());
    let group = Group::on([a.url(), b.url()], "photos")?;
    let mut obs = fava.observe(group.events(Query::events().limit(16)?)?).await?;   // no cache_only
    assert_eq!(sessions_opened(), btreeset![a.key(), b.key()]);   // decoy untouched
    a.serve(shared.clone()); b.serve(shared.clone());
    let projected = group.project(&wait_for(&mut obs)).unwrap();
    assert_eq!(projected.events()[0].relay_evidence.len(), 2);
}
```

**confidence** confirmed

## Conforming (verified, not merely unexamined)

- **`fava-signer` vocabulary.** `Signer`, `SignerAvailability`, `SignerError`, and
  `fava_signer_local::LocalSigner` are exactly the four symbols approved at
  `docs/internals/vocabulary.toml:764-767`. No unapproved public nominal type is exported by either
  signer crate. The added `cancel: watch::Receiver<bool>` parameter on `sign_event` is not a
  deviation: ID-006 (`…GOALS:1219`) explicitly requires signer providers to expose "cancellation".
- **`fava-signer-local` key custody.** `LocalSigner` (`crates/fava-signer-local/src/lib.rs:14-16`)
  holds `Keys` in a private field, derives no `Debug`/`Clone`/`Serialize`, never logs, and exposes no
  accessor — ID-008's "MUST NOT enter … logs, debug formatting, persistent caches" holds at this
  boundary. It also enforces the author binding itself (`lib.rs:40-44`, refusing a mismatched
  `pubkey` as `InvalidOutput`) and honours the cancel receiver with a `biased` select
  (`lib.rs:45-55`). It does **not** bypass the contract: it is reached only through
  `dyn Signer` and adds no side channel. `fava-signer-local`'s dependency set is
  `fava-signer`, `fava-write`, `nostr`, `tokio` — no engine or lifecycle crate.
- **`fava-signer` dependency direction.** `fava-signer` depends only on `fava-write`, `thiserror`,
  `tokio` — a neutral contract crate with no provider, owner, or facade edge.
- **Protocol crates own protocol semantics only.** `fava-nip02` and `fava-simple-groups` depend on
  exactly `fava-query`, `fava-state`, `fava-write`; `fava-bookmarks` on `fava-state`, `fava-write`.
  None depends on `fava`, `fava-observe`, `fava-publication`, `fava-routing`, `fava-write-store`,
  `fava-publisher`, `fava-delivery`, `fava-transport`, or `fava-signer`. I re-ran the check by hand
  rather than trusting the crates' own grep tests.
- **No acquisition or lifecycle inside the protocol crates.** `fava_nip02::{contact_list,
  followers_of}` and `SimpleGroups::{saved_groups, saved_relays, groups_where_admin,
  groups_where_member}` / `Group::{events, records}` return inert `Query` values;
  `follows_of`, `Group::project`, and `SimpleGroups::groups_saved_by` are pure snapshot projections.
  No crate calls `observe`, spawns a task, or holds an `Arc<Mutex<…>>`.
- **No private lifecycle-owner nouns in scope** (the `check_vocabulary.py` blind spot the brief
  flags). The complete set of non-`pub` types across the five crates is:
  `fava-nip02`: `sealed::Sealed`, `Operation`, `Change`, `BoundedTargetText`, `Nip02Materializer`;
  `fava-simple-groups`: `Change`, `Input<'a>`, `SavedListMaterializer`, `GroupOperation<'a>`,
  `RecordBoundary<'a>`, `HostRecords`, `Selected<T>`, `ParsedRecord`, `IntoRelayUrl`, `PreparePayload`;
  `fava-bookmarks`: `Operation`, `Target`, `Change`, `BookmarkMaterializer`.
  Every one is a pure value, a sealed input-conversion trait, or a private implementation of the
  already-approved `ReplaceableEventMaterializer` contract. None owns mutable state, a task, a
  channel, or a cancellation scope. `fava-simple-groups/tests/architecture.rs:191-213` independently
  confirms there is no `static`/`OnceLock`/`Mutex`/`Atomic*` in that crate's sources; I verified the
  same by hand for `fava-nip02` and `fava-bookmarks`.
- **Replaceable-event edits use the specified path.** `fava_nip02::{follow, follow_with, unfollow}`
  (`edit.rs:104-107`), `SimpleGroups::{save_group, remove_group, rename_saved_group, save_relay,
  remove_relay}` (`edit.rs:112-114`), and `fava_bookmarks::{bookmark_event, unbookmark_event,
  bookmark_coordinate, unbookmark_coordinate}` (`lib.rs:119-122`) all construct
  `ReplaceableEventEdit::new(kind, identifier, change)` and each crate supplies an
  `Arc<dyn ReplaceableEventMaterializer>` via `materializer()`. Nothing bypasses `fava-publication`.
- **NIP-29 management events stay author-bearing.** `Group::edit_metadata`/`set_pins`
  (`management.rs:11-36`) produce `UnsignedEvent`, not edits, matching
  `docs/spec/ARCHITECTURE.md:2013-2018`; `Group::prepare` is idempotent and never mutates a
  pre-signed event's identity.
- **Approved simple-groups public vocabulary is exact.** The 14 exports at
  `crates/fava-simple-groups/src/lib.rs:18-24` equal the approved list at
  `docs/spec/ARCHITECTURE.md:1961-1975` and `docs/internals/vocabulary.toml:201-215`, no more and no
  fewer.
- **Bounds are explicit and refuse rather than truncate.** `MAX_GROUP_HOST_INPUT_ITEMS=256`,
  `MAX_GROUP_ID_BYTES=4096`, `MAX_GROUP_QUERY_RESULTS=4096`, `MAX_DISCOVERY_INPUT_ITEMS=256`
  (`fava-simple-groups/src/bounds.rs`) with `collect_at_most` consuming at most `max+1` and returning
  a typed `GroupError::TooMany*`; `GroupSnapshot::project` applies the 4,096 bound **before**
  deduplication (`snapshot.rs:90-96`), exactly as `docs/spec/ARCHITECTURE.md:1993-1995` requires.
  `MAX_TAGS=2000` plus byte-accurate `encoded_len` in both `fava-nip02/src/bounds.rs` and
  `fava-bookmarks/src/bounds.rs`.
- **Stale materialization guards do exist at the store.** `install_signed` and
  `record_signer_refusal` both call `validate_current_materialization`
  (`crates/fava-write-store-memory/src/lifecycle.rs:33, 71`) and `install_signed` verifies the
  signature and exact unsigned body before installing — a stale signer completion cannot *install*
  stale state. The defect (`stale-signer-completion-is-silent`) is that the rejection is unattributed,
  not that stale state gets installed.

## Open questions

1. **`fava_bookmarks` function names.** `docs/spec/ARCHITECTURE.md:747-748` names
   `fava_bookmarks::add(target)` / `remove(target)`; the crate ships `bookmark_event`,
   `unbookmark_event`, `bookmark_coordinate`, `unbookmark_coordinate`. PROTO-003 says a crate
   "SHOULD expose edits such as … bookmark / unbookmark", so the split may be a deliberate
   refinement. Not reported as a finding — no observable behavioral difference.
2. **`fava-bookmarks` has no read surface.** It does not depend on `fava-query` and exposes no
   `decode`/`query`. `docs/spec/ARCHITECTURE.md:1888-1898` says a protocol crate *may* expose those,
   and `docs/internals/vocabulary.toml:186` lists `symbols = []` for `BookmarkList` — so the absence
   looks sanctioned today, but ID-007/private-bookmark support cannot land without both a query
   surface and the missing `decrypt` contract.
3. **`Group::on` silently deduplicates hosts** (`group.rs:317-321`) after `RelayUrl::parse`
   normalization. `docs/spec/ARCHITECTURE.md:2024` says "No helper silently truncates a host, record,
   row, or conflict." Two distinct input strings that normalize to one URL are dropped without a
   typed shortfall. I read this as normalization rather than truncation (`hosts()` is documented as
   "the complete **normalized** host route"), so I did not raise it — worth a ruling.
4. **Severity of `nip02-accepts-bech32-target`.** ID-004 is a named behavioral requirement, which
   would make it `critical` under the brief's rule. I filed it `major` because "wrong identity shape"
   could be read as covering only mis-shaped values (an `nprofile` where a pubkey is required) rather
   than a correctly-shaped bech32 encoding of the right value. A ruling would settle it.
5. **Signing tasks are unsupervised.** `crates/fava-publication/src/run.rs:437` uses a bare
   `tokio::spawn` with no join handle, so a hung signer also has no shutdown join
   (`docs/spec/ARCHITECTURE.md:2355` assigns "resource joining and shutdown deadlines" to the
   non-existent `fava-runtime`). Left to the runtime-absence auditor; noted because it compounds
   `signer-no-deadline-no-timed-out`.
