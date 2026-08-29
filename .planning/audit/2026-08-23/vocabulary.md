# Vocabulary audit

Area slug: `vocabulary`. READ-ONLY. No production source, test, spec, checker, or
registry file was modified.

> Writing note: this report deliberately never places a bare `fava-<token>` that
> is not already registered, and never starts a line with a public nominal Rust
> declaration. Reason: `tools/check_vocabulary.py` ingests `.planning/**/*.md` as
> vocabulary authority (see `vocab-planning-md-is-authority` below), so an audit
> report can itself turn the gate red. Two package names are therefore written
> split.

## Scope checked

- `tools/check_vocabulary.py` (all 306 lines, read in full)
- `tools/tests/test_vocabulary_check.py` (all 222 lines, read in full)
- `tools/tests/` siblings enumerated (`test_nip02_contact_list_feature.py`,
  `test_publication_door_feature.py`, `test_semantic_write_feature.py` — none
  exercise the vocabulary gate)
- `docs/internals/vocabulary.toml` (61 terms, 135 registered symbols, 37
  registered crates, 16 spec symbols, 24 spec crates) — parsed programmatically
- `AGENTS.md:51-60` ("Architectural vocabulary")
- Every `.rs` file under `crates/*/src/**` (210 nominal declarations enumerated
  mechanically), plus `apps/canary/**`, `falsifiers/**`, `crates/*/tests/**`
- `Cargo.toml` workspace members; `.github/workflows/architecture.yml`
- Live runs of `python3 tools/check_vocabulary.py`

Method: the inventory below was produced by a script that applies a
visibility-agnostic declaration regex to every file the crate walker visits,
then re-runs the checker's own `PUBLIC_NOUN` regex over the same lines to
compute exactly which declarations the gate does and does not see. Nine matches
were removed by hand after inspection because they are associated items inside
`impl` blocks, not declarations (eight `IntoIter` in
`crates/fava-nip02/src/query.rs`, one `Item` in
`crates/fava-simple-groups/src/tests/saved.rs:105`).

---

## 1. Exact scope of the gate vs. the written policy

### What the policy requires (`AGENTS.md:53-56`)

- `:53` — "Architectural vocabulary is closed by default. `docs/internals/vocabulary.toml` is the source of truth for concepts, public Rust symbols, specified public Rust symbols, and crate names."
- `:55` — "A new crate, public or cross-crate nominal type, provider contract, persisted entity, configuration concept, or lifecycle owner is a vocabulary change."
- `:56` — "A synonym, wrapper, alternate representation, or adjective-qualified variant of an existing noun is also a vocabulary change."

### What the gate actually matches

`tools/check_vocabulary.py:14-17`:

```
PUBLIC_NOUN = re.compile(
    r"^\s*pub\s+(?:unsafe\s+)?(?:struct|enum|trait|type)\s+([A-Z][A-Za-z0-9_]*)",
    re.MULTILINE,
)
```

`tools/check_vocabulary.py:150-165` bounds where that regex is applied:

```
    for manifest in sorted(crates_root.glob("*/Cargo.toml")):
        ...
        crates.add(package)
        rust_crate = package.replace("-", "_")
        for source in sorted((manifest.parent / "src").rglob("*.rs")):
            ...
            symbols.update(f"{rust_crate}::{name}" for name in PUBLIC_NOUN.findall(text))
```

So the gate sees a declaration only when **all** of these hold:

1. the file is under `crates/<single-segment>/src/**.rs`;
2. the line begins (after whitespace only) with the literal token `pub`,
   optionally `unsafe`, then one of exactly `struct|enum|trait|type`;
3. the name starts with an uppercase ASCII letter.

The symbol key is `<crate>::<Name>` — the module path is discarded
(`:164`), so the gate cannot distinguish two same-named types in different
private modules of one crate, and it treats a `pub` type inside a private module
as if it were reachable public API.

### Categories of policy-covered declaration the gate cannot see

| # | Category | Present today | Count |
|---|---|---|---|
| A | `pub(crate)` nominal declarations | yes | 3 |
| B | `pub(super)` nominal declarations | yes | 7 |
| C | `pub(in path)` nominal declarations | no | 0 (latent) |
| D | private (no visibility) nominal declarations | yes | 56 |
| E | Rust outside `crates/*/src` — `apps/canary/src`, `falsifiers/*/{src,tests}`, `crates/*/tests/**` | yes | 21 public nominal decls |
| F | crates whose manifest is not at `crates/<name>/Cargo.toml` | yes | 3 packages |
| G | `pub use` re-exports that create a public API name | yes | 7 names with no term at all |
| H | `pub union` | no | 0 (keyword absent from the alternation) |
| I | macro-generated / derive-generated nominal types | no | 0 (latent; no `macro_rules!` in `crates/*/src`) |
| J | `pub mod` namespaces, `pub const`/`pub static` configuration concepts | yes | not nominal types, but `:55` covers "configuration concept" |
| K | declarations split across lines (`pub` and `struct` on different lines) | no | 0 (latent) |

Counts A–D exclude the nine associated-item false matches described above.
A + B + C + D = **66 policy-covered declarations inside the gate's own walk root
that the gate never looks at.**

A second, narrower blindness inside the part the gate *does* see:
`closest_registered_noun` (`:231-240`) compares only against **term names**
(`term["name"]`), never against the 135 registered **symbol** leaf names. So a
declaration such as an adjective-qualified variant of the registered symbol
`fava_write::Receipt` gets no "existing noun" hint even when it is flagged,
because `Receipt` is a symbol of the term `Write`, not a term name.

---

## 2. Full inventory of policy-covered, gate-invisible declarations

66 declarations. Classification: **plain** = ordinary internal data with no
architectural noun claim; **wrap** = synonym / wrapper / alternate
representation / adjective-qualified variant of an existing vocabulary noun
(violates `AGENTS.md:56`); **life** = lifecycle owner (violates `AGENTS.md:55`).

**Totals: 13 plain, 44 wrap, 9 life → 53 violations.**

### 2.1 Lifecycle owners (9) — all violations

| file:line | vis | name | why it owns a lifecycle |
|---|---|---|---|
| `crates/fava/src/relay.rs:17` | `pub(super)` | `OpenedRelay` | owns relay session, subscription ids, generation counter (known baseline) |
| `crates/fava/src/query_source.rs:57` | private | `FavaChanges` | owns `cancel: watch::Sender<bool>` for a `tokio::spawn`ed task (`:22-25`), `close()` at `:73`, `Drop` at `:81` |
| `crates/fava-publication/src/revision.rs:17` | `pub(super)` | `OpenedSemanticSources` | owns two `OpenedQuerySource`, releases both in `close()` at `:81-84` |
| `crates/fava-publication/src/revision.rs:87` | `pub(super)` | `PreparedSemantic` | holds `sources: OpenedSemanticSources`; transitively owns the release |
| `crates/fava-publication/src/revision.rs:94` | `pub(super)` | `SemanticState` | holds `sources: OpenedSemanticSources`; `close()` at `:142` |
| `crates/fava-routing/src/chain.rs:110` | private | `OpenedChain` | owns `cancel` for two `tokio::spawn`ed tasks (`:86`, `:95`), `close()` `:132`, `Drop` `:141` |
| `crates/fava-router-outbox/src/lib.rs:180` | private | `OutboxSession` | owns `changes: Option<Box<dyn SourceChanges>>` and releases it in `close()` `:233` |
| `crates/fava-router-outbox/src/lib.rs:31` | private | `KnownLists` | sole authority for a shared mutable relay-list map plus its revision channel, handed around as `Arc<KnownLists>` |
| `crates/fava-transport-websocket/src/lib.rs:82` | private | `WebSocketRelaySession` | owns the split socket sink/stream and the `closed: AtomicBool` (`:82-89`, `:172-177`) |

### 2.2 Synonyms / wrappers / alternate representations (44) — all violations

| file:line | vis | name |
|---|---|---|
| `crates/fava/src/publication.rs:227` | `pub(crate)` | `PublishPayload` |
| `crates/fava/src/routes.rs:64` | private | `Providers` |
| `crates/fava-bookmarks/src/lib.rs:93` | private | `Operation` |
| `crates/fava-bookmarks/src/lib.rs:108` | private | `Target` |
| `crates/fava-bookmarks/src/lib.rs:114` | private | `Change` |
| `crates/fava-bookmarks/src/lib.rs:259` | private | `BookmarkApplier` |
| `crates/fava-diagnostics/src/lib.rs:10` | private | `SessionFact` |
| `crates/fava-diagnostics/src/lib.rs:11` | private | `SubscriptionFact` |
| `crates/fava-diagnostics/src/lib.rs:12` | private | `MessageFact` |
| `crates/fava-diagnostics/src/lib.rs:13` | private | `FailureFact` |
| `crates/fava-event-cache-memory/src/lib.rs:148` | private | `WatchChanges` |
| `crates/fava-nip02/src/edit.rs:79` | private | `Operation` |
| `crates/fava-nip02/src/edit.rs:99` | private | `Change` |
| `crates/fava-nip02/src/edit.rs:284` | private | `Nip02Applier` |
| `crates/fava-observe/src/lib.rs:286` | private | `TrackingSource` |
| `crates/fava-observe/src/lib.rs:307` | private | `TrackingChanges` |
| `crates/fava-observe/src/lib.rs:321` | private | `RefusingSource` |
| `crates/fava-observe/src/lib.rs:331` | private | `EmptyEvaluator` |
| `crates/fava-observe/src/lib.rs:343` | private | `FailingEvaluator` |
| `crates/fava-router-app-relays/src/lib.rs:104` | private | `StaticSession` |
| `crates/fava-router-fallback-relays/src/lib.rs:135` | private | `FallbackSession` |
| `crates/fava-router-hints/src/lib.rs:138` | private | `HintSession` |
| `crates/fava-router-testkit/src/lib.rs:69` | private | `DelayedSession` |
| `crates/fava-routing/src/chain.rs:146` | private | `RouterUpdate` |
| `crates/fava-simple-groups/src/edit.rs:20` | private | `Change` |
| `crates/fava-simple-groups/src/edit.rs:298` | private | `SavedListApplier` |
| `crates/fava-simple-groups/src/edit.rs:370` | private | `GroupOperation` |
| `crates/fava-simple-groups/src/group.rs:376` | private | `IntoRelayUrl` |
| `crates/fava-simple-groups/src/group.rs:404` | private | `PreparePayload` |
| `crates/fava-simple-groups/src/records.rs:10` | `pub(crate)` | `RecordBoundary` |
| `crates/fava-simple-groups/src/snapshot.rs:14` | private | `HostRecords` |
| `crates/fava-simple-groups/src/snapshot.rs:52` | private | `Selected<T>` |
| `crates/fava-simple-groups/src/snapshot.rs:59` | private | `ParsedRecord` |
| `crates/fava-subscriptions-no-grouping/src/lib.rs:11` | private | `OnePerDemand` |
| `crates/fava-subscriptions-standard/src/lib.rs:101` | private | `Group` |
| `crates/fava-write/src/edit.rs:117` | private | `EncodedEdit` |
| `crates/fava-write-store-memory/src/lib.rs:404` | private | `WatchChanges` |
| `crates/fava-write-store-memory/src/model.rs:40` | `pub(super)` | `UnsignedEventView` |
| `crates/fava-write-store-memory/src/semantic.rs:24` | `pub(super)` | `WriteState` |
| `crates/fava-write-store-redb/src/lib.rs:30` | private | `SemanticCustody` |
| `crates/fava-write-store-redb/src/lib.rs:158` | private | `WatchChanges` |
| `crates/fava-write-store-redb/src/lifecycle.rs:144` | `pub(super)` | `UnsignedEventView` |
| `crates/fava-write-store-redb/src/schema.rs:18` | private | `PersistedReceipt` |
| `crates/fava-write-store-redb/src/schema.rs:24` | private | `PersistedSemantic` |

### 2.3 Plain data (13) — conforming, no vocabulary entry needed

`crates/fava-diagnostics/src/lib.rs:49` `State`;
`crates/fava-event-cache-memory/src/lib.rs:24` `CacheState`;
`crates/fava-nip02/src/edit.rs:242` `BoundedTargetText`;
`crates/fava-nip02/src/query.rs:9` `Sealed` (`pub(crate)`, idiomatic sealing marker);
`crates/fava-nip02/src/tests/edit.rs:113` `HostileTarget` (test fixture);
`crates/fava-simple-groups/src/edit.rs:212` `Input<'a>` (byte cursor);
`crates/fava-simple-groups/src/tests/saved.rs:99` `PanicAfter` (test fixture);
`crates/fava-simple-groups/src/tests/snapshot.rs:48` `ProjectionOutcome` (test helper trait);
`crates/fava-transport-websocket/src/lib.rs:16,17,18` `Socket`, `SocketSink`, `SocketStream` (mechanism aliases over `tokio_tungstenite` types);
`crates/fava-write-store-redb/src/lib.rs:47` `StoreState`;
`crates/fava-write-store-redb/src/lib.rs:58` `StoreLimits`.

---

## 3. Where each violation should collapse

Grouped by the collision, because the same unnamed concept recurs across crates.

**3.1 The `SourceChanges` implementations — 4 spellings of one noun.**
`FavaChanges`, and `WatchChanges` in three separate crates
(`fava-event-cache-memory:148`, `fava-write-store-memory:404`,
`fava-write-store-redb:158`) are four private structs implementing the
registered contract `fava_query::SourceChanges`. Collapse: they are the
close-side of an opened source; either register one shared noun (e.g. a
`SourceChanges` handle owned by `fava-query`) or name them consistently as
implementations the way `MemoryEventCache` / `RedbWriteStore` /
`StandardQueryEvaluator` are already registered. Three byte-identical
declarations in three crates is a cohesion signal, not three concepts.

**3.2 The `RouterSession` implementations — 6 unregistered `*Session` nouns.**
`StaticSession`, `FallbackSession`, `HintSession`, `OutboxSession`,
`DelayedSession`, `OpenedChain`. `RouterSession` is a registered symbol
(`fava_routing::RouterSession`) with a registered spec symbol as well; every
other replaceable-contract implementation in the workspace has a registered
noun (`AppRelayRouter`, `OutboxRouter`, `HintRouter`, `FallbackRelayRouter`,
`DelayedRouter`). The session side of the exact same contract has zero. Either
all six get entries under the `Router` term, or the term's meaning is widened to
say implementations' session types are covered. `OpenedChain` additionally
overlaps the `Session` term (currently spec-only, owner `fava-session`).

**3.3 `WebSocketRelaySession` (`fava-transport-websocket:82`).** Adjective-
qualified variant of the registered `fava_transport::RelaySession`. Its sibling
`WebSocketTransport` in the same file *is* registered under the `Transport`
term. Collapse: add it to the `Transport` term's symbol list, exactly as
`WebSocketTransport` already is.

**3.4 `OpenedRelay` (`fava/src/relay.rs:17`).** Known baseline. Needs no new
entry — it needs to stop existing; the relay-session lifecycle belongs to
`fava-transport` + `fava-observe` per the architecture spec. It is the strongest
example of `:55` being violated with a green gate.

**3.5 The publication revision triple (`fava-publication:17,87,94`).**
`OpenedSemanticSources`, `PreparedSemantic`, `SemanticState` are three
`pub(super)` lifecycle owners for one thing: the pair of query sources kept open
while a semantic (replaceable-event) write is applied. Closest existing
concept: `Publication` (registered, owner `fava-publication`). Either they
collapse into `Publication`'s own state, or one real entry is needed — the
forcing requirement is that a semantic write must hold two open sources across
retries, which `Publication` does not currently name. `OpenedSemanticSources` in
particular is the same shape as `OpenedRelay` in a different crate.

**3.6 The three semantic-edit codecs.** `Change` appears three times
(`fava-bookmarks:114`, `fava-nip02:99`, `fava-simple-groups:20`), `Operation`
twice (`fava-bookmarks:93`, `fava-nip02:79`), plus `Target`
(`fava-bookmarks:108`) and `GroupOperation` (`fava-simple-groups:370`). All are
the decoded body of the registered `fava_write::EventEdit.change`
byte field. Collapse into `EventEdit` — the concept is already
approved; what is missing is one noun for its decoded form. Today the same
architectural concept has three private homonyms across three crates, which is
precisely what `:56` exists to prevent.

**3.7 The three `EditApplier` implementations.**
`BookmarkApplier`, `Nip02Applier`, `SavedListApplier`. The trait
is a registered symbol and a registered spec symbol; every one of its
implementations is unregistered because each crate exposes only
`pub fn applier() -> Arc<dyn EditApplier>`. This is a
deliberate-looking gate evasion pattern: return the contract, keep the noun
private. Collapse: register the three under the
`EditApplier` term, consistent with `Nip01Publisher` /
`StandardDeliveryPolicy` / `LocalSigner` all being registered.

**3.8 `OnePerDemand` (`fava-subscriptions-no-grouping:11`).** Same pattern as
3.7 and the sharpest single case: the crate `fava-subscriptions-no-grouping`
*is* registered under the `SubscriptionPlanner` term, and its sibling crate's
planner `StandardSubscriptionPlanner` *is* a registered symbol — but this
crate's sole planner implementation has no name in the registry at all, only
because it is private. Needs a registry entry under `SubscriptionPlanner`.

**3.9 `Group` (`fava-subscriptions-standard:101`) — homonym collision.** This is
a coalesced-subscription grouping. `Group` is already an approved term meaning a
NIP-29 relay-hosted group owned by `fava-simple-groups`. Two unrelated meanings,
one spelling, in one workspace. Must be renamed into the `SubscriptionPlanner`
family (`SubscriptionGroup`, or collapse into `SubscriptionPlan` / `RelayDemand`
which are both registered).

**3.10 The three "payload" traits.** `PublishPayload`
(`fava/src/publication.rs:227`, `pub(crate)` — cross-module), `PreparePayload`
(`fava-simple-groups/group.rs:404`), against the registered
`fava_write::WritePayload` (also a registered spec symbol). Three names, one
"turn a caller value into a write" idea. Collapse both into `WritePayload`.

**3.11 `IntoRelayUrl` (`fava-simple-groups/group.rs:376`).** Structurally
identical to `fava_nip02::IntoContactAuthors`, which *is* a registered symbol
under the `IntoContactAuthors` term. One conversion trait registered, its twin
not — solely because one is `pub` and the other is not. Register it, or state
that input-conversion traits are exempt (and then remove the nip02 entry).

**3.12 The persisted redb entities.** `PersistedReceipt`
(`fava-write-store-redb/schema.rs:18`) and `PersistedSemantic` (`:24`) are the
on-disk serde schema records behind `SCHEMA_VERSION = 2`. `AGENTS.md:55` names
"persisted entity" explicitly as a vocabulary change; these are the only literal
persisted entities in the workspace and neither is registered. `PersistedReceipt`
is an adjective-qualified variant of the registered `fava_write::Receipt`.
Either collapse (persist `Receipt` directly) or give both real entries — a
schema-versioned durable record is exactly the case that needs a named owner.

**3.13 `SemanticCustody` (`fava-write-store-redb:30`) and `WriteState`
(`fava-write-store-memory/semantic.rs:24`, `pub(super)`).** The same four-field
semantic custody tuple exists as a redb type alias and, inline and
`#[allow(clippy::type_complexity)]`-suppressed, as the `edits` map of
`WriteState` in the memory store. One concept, two crates, no name. Needs a real
entry under `WriteStore` (or collapse into `fava_write::Receipt`'s semantic
attachment).

**3.14 `UnsignedEventView` — declared twice, byte-identical.**
`fava-write-store-memory/model.rs:40` and
`fava-write-store-redb/lifecycle.rs:144`, both `pub(super)`, both six identical
fields. Alternate representation of the registered spec symbol `UnsignedEvent`
(a term with **no** registered code symbol at all). Collapse into one shared
borrowed view owned by `fava-write`, then register it under `UnsignedEvent`.

**3.15 The diagnostics fact aliases.** `SessionFact`, `SubscriptionFact`,
`MessageFact`, `FailureFact` (`fava-diagnostics:10-13`). These are not merely
private: they are the declared field types of the **public**
`DiagnosticsSnapshot` (`:26-38`), so they appear in the crate's rustdoc and in
its public shape while carrying no vocabulary entry. Closest concepts:
`RelayEvidence` and `RelaySessionKey` (both registered, owner `fava-state`).
Collapse into named `RelayEvidence` variants, or register four entries under
`Diagnostics`.

**3.16 `EncodedEdit` (`fava-write/src/edit.rs:117`).** The serde wire form of
the registered `EventEdit`. Textbook "alternate representation".
Collapse into `EventEdit`'s own serde impl.

**3.17 The simple-groups projection internals.** `HostRecords` (`snapshot.rs:14`),
`ParsedRecord` (`:59`), `RecordBoundary` (`records.rs:10`, `pub(crate)`) are
per-host / per-record slices of the registered `GroupRecords` and
`GroupSnapshot`. `Selected<T>` (`:52`) is an `(EventId, Timestamp, T)` triple —
an alternate representation of the registered `fava_query::EventRecord`.
Collapse `Selected<T>` into `EventRecord`; the other three fold under the
`Group` term.

**3.18 `Providers` (`fava/src/routes.rs:64`) and `RouterUpdate`
(`fava-routing/chain.rs:146`).** `Providers` is an unnamed aggregate of four
registered provider contracts (`Transport`, `SubscriptionPlanner`, `EventCache`,
`Diagnostics`); `RouterUpdate` is an alternate representation of the registered
`fava_routing::RouteContribution` plus an index and a name. Both should collapse
into the existing nouns.

**3.19 The `fava-observe` test doubles** (`TrackingSource`, `TrackingChanges`,
`RefusingSource`, `EmptyEvaluator`, `FailingEvaluator`, `lib.rs:286-343`). These
are adjective-qualified variants of `QuerySource`, `SourceChanges`, and
`QueryEvaluator` and are therefore literally covered by `:56`. They are
test-scope and low risk, but there is currently no written exemption for test
doubles — the policy has no test carve-out, so either the policy gains one or
these need entries. Flagged as the lowest-severity members of the 44.

---

## 4. Reverse direction

### 4.1 Stale / unverifiable registry entries

`check()` verifies four symmetric pairs (`:254-284`). Two of them are real
round-trips (`registry.symbols` vs. code, `registry.crates` vs. packages) and
both currently balance exactly: **0** registered symbols missing from code, **0**
public symbols missing from the registry, **0** registered crates missing, **0**
packages unregistered. That half of the gate is genuinely tight.

The other two pairs are **not** round-trips against reality. `spec_symbols` and
`spec_crates` are only ever checked against *prose in `docs/spec/*.md` and
`.planning/**/*.md`* (`:205-228`, `:277-284`). Nothing ever asks whether a
`spec_crate` corresponds to a crate that exists, or whether a `spec_symbol`
corresponds to a declaration that exists. Consequence: **24 of the 24
`spec_crates` do not exist as packages** and the gate is silent about all of
them —

`fava-nip18`, `fava-nip22`, `fava-nip25`, `fava-ffi`, `fava-runtime`,
`fava-standard`, `fava-event-cache-redb`, `fava-event-cache-testkit`,
`fava-write-store-testkit`, `fava-subscriptions-testkit`, `fava-relay-lab`,
`fava-publisher-testkit`, `fava-signer-nip46`, `fava-signer-testkit`,
`fava-session`, `fava-fetch-cache`, `fava-fetch-cache-memory`,
`fava-fetch-cache-redb`, `fava-nip05`, `fava-nip05-http`, `fava-nip11`,
`fava-nip11-http`, `fava-auth`, `fava-content`.

Two of those (`fava-runtime`, `fava-session`) are exactly the crates the audit
brief names as "do not exist at all". The registry records them as approved
vocabulary and the gate reports success, because their only obligation is that
*a document mentions them*. Likewise **16 of the 16 `spec_symbols`** have no
implementation (`Session`, `SessionError`, `FetchCache`, `Nip05Resolver`,
`RelayInformationFetcher`, `UnsignedEvent`, `ClientMessage`, `RelayMessage`,
`QueryRouting`, `Selection`, `ValueSet`, `QueryChange`, `EventStateDecision`,
`ReadRouteRequest`, `WriteRouteRequest`, `TargetCoverage`).

Nine terms (`Event`, `EventId`, `PublicKey`, `Kind`, `Tag`, `Filter`,
`RelayUrl`, `Subscription`, `Coordinate`) carry no symbols, crates, spec symbols,
or spec crates. Those are legitimately external Nostr nouns; not stale.

Eleven further terms (`RelayMessage`, `Repost`, `Comment`, `Reaction`,
`UnsignedEvent`, `Session`, `FetchCache`, `Nip05Resolver`,
`RelayInformationFetcher`, `Authentication`, `ParsedContent`) have **no real
code anchor at all** — only spec references. These are aspirational vocabulary
carried in a registry that `AGENTS.md:53` calls "the source of truth", and
`AGENTS.md:59` ("Documentation describes the current model only") argues against
keeping them there unqualified.

### 4.2 Public declarations in code, absent from the toml, gate green

**(a) Rust outside the walk root — 21 public nominal declarations, 3 packages.**
`collect_public_symbols` globs `crates/*/Cargo.toml` (`:150`) and walks only
`<manifest>/src` (`:158`). Everything else is unreachable:

- `apps/canary/src/**` — 11: `CanaryError` (`lib.rs:87`), `CanaryResult`
  (`lib.rs:146`), `Scenario` (`:150`), `SmokeOptions` (`:220`), `SmokeOutcome`
  (`:231`), `CroissantSimpleGroupsOptions` / `CroissantSimpleGroupsOutcome`
  (`croissant_simple_groups.rs:32,49`), `CroissantNip02Options` /
  `CroissantNip02Outcome` (`croissant_nip02.rs:56,67`), `ReconOptions` /
  `ReconOutcome` (`recon.rs:14,25`). This package also declares its own
  `[workspace]`, so it is not even a member of the root workspace.
- `falsifiers/external-null-cache/src/lib.rs:12` — `NullEventCache`, a
  competing `EventCache` implementation, i.e. exactly the replaceability proof
  the architecture cares about, and it is invisible to the vocabulary gate.
- `falsifiers/external-semantic-capability/tests/support/mod.rs:33,69` —
  `Harness`, `ScriptedTransport`.
- `crates/fava/tests/support/semantic_write.rs:162,169,229,266,307,332,409` — 7
  public test doubles including `CountingSigner`, `BlockingSigner`,
  `RecordingPublisher`, `CountingRouter`, `NoopTransport`.

The three package names are also invisible to the crate half of the gate:
`canary`, `external-semantic-capability-proof`, and the one whose manifest lives
at `falsifiers/external-null-cache/Cargo.toml` (its package name is the
`fava-` prefix followed by `external-null-cache-proof` — written split here on
purpose, see the writing note). That last one is a `fava-…` crate name that the
crate gate would reject on sight if it appeared in any document, yet the package
itself passes because it is not under `crates/`.

**(b) `pub use` re-exports — 7 public API names with no term.**
The regex matches declarations only. A re-export creates a public name in a
crate's API without any `struct|enum|trait|type` keyword, so the gate never sees
it: `fava::SingleLetterTag`, `fava::Timestamp`, `fava_query::SingleLetterTag`,
`fava_query::Timestamp`, `fava_state::Timestamp`, `fava_wire::SubscriptionId`,
`fava_write::Timestamp`. `Timestamp`, `SingleLetterTag`, and `SubscriptionId`
are not terms in the registry at any level. Note the registry *does* carry terms
for other re-exported Nostr nouns (`EventId`, `Kind`, `PublicKey`, `RelayUrl`,
`ClientMessage`, `RelayMessage`) with empty `symbols` lists — so the intended
model is that re-exported nouns get terms; three simply never did, and nothing
can detect it.

**(c) Latent, zero instances today but unguarded.** `pub union` (keyword absent
from the alternation at `:15`); macro-generated nominal types; a declaration
whose `pub` and keyword are on different lines; a crate manifest nested deeper
than one segment under `crates/`.

**(d) Direction of over-inclusion.** Because the symbol key drops the module
path (`:164`), a `pub` type inside a *private* module is registered as though it
were public API. The registry therefore promises reachability it does not
verify. No current entry was found to be unreachable, but the check does not
exist.

---

## 5. Audit of `tools/tests/test_vocabulary_check.py`

Nine test methods, all routed through one `run_check` helper (`:158-215`) that
builds a fixture with exactly one crate (`crates/sample`), one source file
(`src/lib.rs`), one spec file (`docs/spec/ARCHITECTURE.md`), and a one-term
registry.

**What they cover:**
- 2 tests on the code-symbol path: an unregistered symbol is rejected with the
  "existing noun" hint (`:17-25`), a registered one is accepted (`:27-33`).
- 2 tests on the spec-symbol path: reject unregistered (`:35-42`), accept
  registered (`:44-53`).
- 5 tests on `is_structural_crate_metadata` false-positive suppression —
  `/tmp/` paths, phase slug front-matter, phase-numbered directory prefixes,
  linked-worktree prefixes, the checker's own diagnostic string — each with a
  paired control asserting the suppression does not swallow a genuine crate
  reference on the same line.

**What they never touch:**
- **Any visibility other than bare `pub`.** No fixture contains `pub(crate)`,
  `pub(super)`, `pub(in …)`, or a private declaration. There is not even a
  negative test pinning "the gate is intentionally silent about these", so the
  blindness is not a recorded decision — it is an untested gap.
- **Any keyword other than `struct`.** `enum`, `trait`, and `type` appear in the
  regex and in zero fixtures. So does `unsafe`.
- **`load_registry` validation** (`:44-132`) — roughly twenty distinct problem
  branches (version != 1, missing required fields, non-table term, duplicate
  term name, bad `source`, missing `protocol` / `nearest_nostr` / `distinction`,
  non-string `meaning`/`owner`, non-list `symbols`/`crates`/`spec_*`, duplicate
  registered symbol or crate, unreadable/invalid TOML). **Zero** tests. Every
  fixture writes one syntactically perfect term.
- **The real-crate half of the gate.** "undocumented architectural crate" and
  "registered architectural crate does not exist" (`:262-265`) are never
  exercised; only the `spec_crates` messages are.
- **The reverse symbol check.** "registered public symbol does not exist"
  (`:260-261`) and "registered specified symbol does not exist" (`:277-278`) are
  never exercised.
- **Multi-crate roots**, nested manifests, missing `crates/` directory, missing
  `docs/spec/` directory, unreadable source files.
- **`.planning/**` ingestion.** `collect_spec_vocabulary` (`:214-216`) adds every
  `.planning/**/*.md` to the document set; the fixture never creates
  `.planning/`. The entire planning-document authority path is untested.
- **`closest_registered_noun` term-name-only matching** (`:231-240`) is asserted
  once, positively, on a single-word term.

**The class of violation these tests can never catch:** any violation whose
evidence is not a line of the exact form `pub struct Name` in
`crates/sample/src/lib.rs`. The helper's signature makes that structural, not
accidental — `run_check(source=…)` writes one string into one file in one crate
named `sample`. There is no parameter for a second crate, for a file outside
`src`, for a non-`crates/` package, or for a re-export. Therefore a
`pub(super)` lifecycle owner, a duplicated noun across two crates, a public type
in `apps/`, and a `pub use` alias are all *unrepresentable* as test inputs. The
tests can only ever regress-protect the narrow slice the regex already sees.

---

## Findings

### vocab-gate-blind-to-nonpublic-nominals — critical — vocabulary
- **authority** — `AGENTS.md:55`: "A new crate, public or cross-crate nominal type, provider contract, persisted entity, configuration concept, or lifecycle owner is a vocabulary change." and `AGENTS.md:56`: "A synonym, wrapper, alternate representation, or adjective-qualified variant of an existing noun is also a vocabulary change."
- **implementation** — `tools/check_vocabulary.py:14-17` matches `^\s*pub\s+(?:unsafe\s+)?(?:struct|enum|trait|type)\s+…` only. 66 policy-covered declarations inside the gate's own walk root are never examined; 53 of them are violations by the classification in section 2 (9 lifecycle owners, 44 synonyms/wrappers/variants).
- **observable distinction** — `python3 tools/check_vocabulary.py` exits reporting only the unrelated `fava-canary` diagnostic on a tree containing 9 unapproved lifecycle owners. Change any of those 9 declarations to bare `pub` and the same tree produces 9 new failures with no semantic change to the program. The gate's verdict depends on a visibility keyword, not on the concept.
- **proposed falsifier** — `tools/tests/test_vocabulary_check.py::test_rejects_an_undocumented_lifecycle_owner_at_restricted_visibility`: run the helper with a source string declaring a `pub(super)` struct named `OpenedThing` alongside the registered `Query`; assert `returncode != 0` and `"sample::OpenedThing"` in stderr. Fails today (exit 0).
- **confidence** — confirmed.

### vocab-openedrelay-and-eight-siblings — critical — vocabulary
- **authority** — `AGENTS.md:55` (lifecycle owner is a vocabulary change); `AGENTS.md:58`: "Vocabulary changes use a separate focused architecture change approved by Pablo. A feature change cannot approve its own new vocabulary."
- **implementation** — `crates/fava/src/relay.rs:17` (`OpenedRelay`, known baseline) is not unique. Eight further unapproved lifecycle owners exist: `crates/fava/src/query_source.rs:57`, `crates/fava-publication/src/revision.rs:17,87,94`, `crates/fava-routing/src/chain.rs:110`, `crates/fava-router-outbox/src/lib.rs:31,180`, `crates/fava-transport-websocket/src/lib.rs:82`. Each owns a cancel channel, a spawned task, an opened source, or a socket, and each is absent from `docs/internals/vocabulary.toml`.
- **observable distinction** — `OpenedSemanticSources` (`revision.rs:17`) owns two `OpenedQuerySource` handles and is the only thing that closes them (`:81-84`). An application that cancels a semantic publication mid-revision depends on a lifecycle whose owner has no name, no registered owner crate, and no falsifier obligation — the same shape as the confirmed partial-open session leak in the baseline.
- **proposed falsifier** — `crates/fava-publication/tests/semantic_cancellation.rs::cancelled_revision_closes_both_opened_sources`: open a `Publication` for a replaceable edit against two counting `QuerySource`s, drop the publication future before the signer resolves, assert both sources recorded exactly one `close()`. Names the lifecycle at the owning component instead of leaving it anonymous.
- **confidence** — confirmed.

### vocab-provider-impls-hidden-behind-arc-dyn — major — vocabulary
- **authority** — `AGENTS.md:55` ("provider contract … is a vocabulary change"); `AGENTS.md:53` (registry is the source of truth for public Rust symbols).
- **implementation** — Every provider implementation reached through a `pub` type is registered (`fava_query_standard::StandardQueryEvaluator`, `fava_publisher_nip01::Nip01Publisher`, `fava_signer_local::LocalSigner`, `fava_router_*::*Router`, `fava_subscriptions_standard::StandardSubscriptionPlanner`). Every provider implementation reached through `pub fn …() -> Arc<dyn Contract>` is unregistered: `crates/fava-bookmarks/src/lib.rs:259`, `crates/fava-nip02/src/edit.rs:284`, `crates/fava-simple-groups/src/edit.rs:298`, `crates/fava-subscriptions-no-grouping/src/lib.rs:11`. `fava-subscriptions-no-grouping` is a **registered crate** whose only planner has no registered noun.
- **observable distinction** — a competing implementer reading `vocabulary.toml` to find the approved set of `SubscriptionPlanner` implementations sees `StandardSubscriptionPlanner` and nothing for the no-grouping crate, although both are shipped defaults reachable from the public API. The registry under-reports the delivered provider surface.
- **proposed falsifier** — `tools/tests/test_vocabulary_check.py::test_rejects_a_private_provider_implementation_returned_as_a_contract`: fixture source declaring a private struct `HiddenPlanner` plus `pub fn planner() -> Arc<dyn Planner>`; assert the gate names `sample::HiddenPlanner`. Fails today.
- **confidence** — confirmed.

### vocab-persisted-entities-unregistered — major — vocabulary
- **authority** — `AGENTS.md:55`: "… persisted entity … is a vocabulary change."
- **implementation** — `crates/fava-write-store-redb/src/schema.rs:18` (`PersistedReceipt`) and `:24` (`PersistedSemantic`) are the serde records written to redb behind `SCHEMA_VERSION: u64 = 2` (`:15`). Neither appears in `docs/internals/vocabulary.toml`. `PersistedReceipt` is an adjective-qualified variant of the registered `fava_write::Receipt`.
- **observable distinction** — these two shapes define durable on-disk compatibility across restarts; a schema change silently alters what survives a process kill. The workspace's only literal persisted entities have no named owner and no approval record, so no falsifier is attached to their evolution.
- **proposed falsifier** — `crates/fava-write-store-redb/tests/process_kill/schema.rs::persisted_receipt_schema_is_pinned`: write one semantic receipt, reopen the database with a fixture file produced at `SCHEMA_VERSION = 2`, assert the recovered `Receipt` and its semantic attachment match field for field.
- **confidence** — confirmed.

### vocab-duplicate-nouns-across-crates — major — vocabulary
- **authority** — `AGENTS.md:56` (synonym / alternate representation is a vocabulary change); `AGENTS.md:53` (closed by default).
- **implementation** — `UnsignedEventView` declared byte-identically at `crates/fava-write-store-memory/src/model.rs:40` and `crates/fava-write-store-redb/src/lifecycle.rs:144`. `WatchChanges` declared three times: `crates/fava-event-cache-memory/src/lib.rs:148`, `crates/fava-write-store-memory/src/lib.rs:404`, `crates/fava-write-store-redb/src/lib.rs:158`. `Change` declared three times: `crates/fava-bookmarks/src/lib.rs:114`, `crates/fava-nip02/src/edit.rs:99`, `crates/fava-simple-groups/src/edit.rs:20`. `Operation` twice: `crates/fava-bookmarks/src/lib.rs:93`, `crates/fava-nip02/src/edit.rs:79`. The semantic custody tuple exists twice in two spellings: `crates/fava-write-store-redb/src/lib.rs:30` and inline in `crates/fava-write-store-memory/src/semantic.rs:24`.
- **observable distinction** — an alternative `WriteStore` implementor must reinvent `UnsignedEventView` and the custody tuple from scratch because neither is a named, exported concept; the two shipped stores agree only by copy. A divergence between them is invisible to every gate.
- **proposed falsifier** — `crates/fava-write-store/tests/parity.rs::memory_and_redb_agree_on_unsigned_event_identity`: build one `UnsignedEvent`, feed it to both stores through the public `WriteStore` contract, assert both derive the same identity and coordinate. Fails the moment the two private copies drift.
- **confidence** — confirmed.

### vocab-group-homonym — major — vocabulary
- **authority** — `AGENTS.md:56`; `docs/internals/vocabulary.toml` term `Group`, `source = "nostr"`, `owner = "fava-simple-groups"`.
- **implementation** — `crates/fava-subscriptions-standard/src/lib.rs:101` declares a private struct named `Group` holding `wire_id`, `filter`, `logical` — a coalesced subscription group, unrelated to the approved NIP-29 `Group`. One spelling, two meanings, one workspace.
- **observable distinction** — the checker's own `closest_registered_noun` matches this name to the term `Group` exactly (verified by running `check_vocabulary.closest_registered_noun` against it), so the tool already considers it a collision; only the visibility keyword stops it being reported.
- **proposed falsifier** — the same widened-visibility gate test as `vocab-gate-blind-to-nonpublic-nominals`; this declaration is one of the 16 that the checker's existing noun heuristic would flag by name the moment visibility stops filtering.
- **confidence** — confirmed.

### vocab-diagnostics-facts-leak-into-public-shape — major — vocabulary
- **authority** — `AGENTS.md:55` ("public … nominal type"); `AGENTS.md:56`.
- **implementation** — `crates/fava-diagnostics/src/lib.rs:10-13` declare four private tuple aliases (`SessionFact`, `SubscriptionFact`, `MessageFact`, `FailureFact`) that are the declared field types of the **public** `DiagnosticsSnapshot` at `:26-38`. `DiagnosticsSnapshot` is registered; its field vocabulary is not. `SessionFact` and `SubscriptionFact` are adjective-qualified variants of the terms `Session` and `Subscription`.
- **observable distinction** — the crate's public rustdoc renders four undefined nouns; an application reading `Fava::diagnostics()` sees tuple shapes with no named meaning and no owner, while `RelayEvidence` and `RelaySessionKey` — the approved nouns for exactly these facts — go unused.
- **proposed falsifier** — `crates/fava-diagnostics/tests/vocabulary.rs::snapshot_fields_use_named_relay_evidence`: assert `DiagnosticsSnapshot::sessions` yields `RelayObservation`-shaped values rather than anonymous tuples. Fails today.
- **confidence** — confirmed.

### vocab-walk-root-excludes-three-packages — major — vocabulary
- **authority** — `AGENTS.md:53`: the registry "is the source of truth for concepts, public Rust symbols, specified public Rust symbols, and **crate names**."
- **implementation** — `tools/check_vocabulary.py:150` globs `crates/*/Cargo.toml` and `:158` walks only `<manifest>/src`. Three packages are outside that: `apps/canary` (package `canary`, and it declares its own `[workspace]`), `falsifiers/external-semantic-capability` (package `external-semantic-capability-proof`), and `falsifiers/external-null-cache` (package name = `fava-` prefix + `external-null-cache-proof`). Together with `crates/*/tests/**` they contain 21 `pub` nominal declarations, itemised in section 4.2(a) — including `NullEventCache` at `falsifiers/external-null-cache/src/lib.rs:12`, a competing `EventCache` implementation, i.e. the replaceability proof itself.
- **observable distinction** — the last package's name is a `fava-…` token that the crate half of the gate rejects on sight when it appears in any document, yet the package itself ships unregistered. The gate's crate authority is decided by directory layout, not by the workspace.
- **proposed falsifier** — `tools/tests/test_vocabulary_check.py::test_scans_every_workspace_member_not_only_crates_dir`: fixture with a root `Cargo.toml` listing `crates/sample` and `apps/tool`, an unregistered public type in `apps/tool/src/lib.rs`; assert non-zero exit naming it. Fails today (exit 0).
- **confidence** — confirmed.

### vocab-spec-side-never-checked-against-reality — major — vocabulary
- **authority** — `AGENTS.md:53` (source of truth) and `AGENTS.md:59`: "Documentation describes the current model only. Replace superseded concepts completely; do not retain migration narration, aliases, or rejected-design commentary in authoritative docs or code."
- **implementation** — `tools/check_vocabulary.py:277-284` compares `registry.spec_symbols` and `registry.spec_crates` only against tokens found in `docs/spec/*.md` and `.planning/**/*.md` (`:205-228`). Nothing compares them to code. Result: **24/24 registered `spec_crates` name packages that do not exist** (list in section 4.1, including `fava-runtime` and `fava-session`, which the brief confirms do not exist), and **16/16 registered `spec_symbols` have no implementation**. The gate reports success.
- **observable distinction** — the registry asserts that `Session`, `SessionError`, and the `fava-session` crate are approved architecture; an application looking for them finds nothing, and no gate distinguishes "approved and delivered" from "approved and absent". The vocabulary cannot be used to tell what exists.
- **proposed falsifier** — `tools/tests/test_vocabulary_check.py::test_reports_specified_vocabulary_with_no_implementation`: registry with `spec_crates = ["sample-missing"]` mentioned in the spec but absent from the workspace; assert the gate emits an "unimplemented specified crate" diagnostic (a warning-class message, distinct from a violation, if intentional aspiration is allowed).
- **confidence** — confirmed.

### vocab-reexports-invisible — minor — vocabulary
- **authority** — `AGENTS.md:53` (source of truth for public Rust symbols).
- **implementation** — the regex matches declarations, so a `pub use` never registers. Seven public API names have no term at any level: `fava::SingleLetterTag` and `fava::Timestamp` (`crates/fava/src/lib.rs:21,28`), `fava_query::SingleLetterTag` and `fava_query::Timestamp` (`crates/fava-query/src/lib.rs:15,16`), `fava_state::Timestamp`, `fava_wire::SubscriptionId` (`crates/fava-wire/src/lib.rs:3`), `fava_write::Timestamp`. The registry demonstrably intends re-exported Nostr nouns to have terms — `EventId`, `Kind`, `PublicKey`, `RelayUrl`, `ClientMessage`, `RelayMessage` all do, with empty `symbols` lists — so this is an omission the gate cannot detect, not a deliberate exclusion.
- **observable distinction** — `fava::Timestamp` is part of the crate's public API surface and appears in every query result signature, with no approved meaning.
- **proposed falsifier** — `tools/tests/test_vocabulary_check.py::test_rejects_an_unregistered_public_reexport`: fixture with `pub use nostr::Timestamp;` and no matching term; assert non-zero exit.
- **confidence** — confirmed.

### vocab-planning-md-is-authority — minor — vocabulary
- **authority** — audit brief authority order: "`.planning/` records … are NOT authority."; `AGENTS.md:59`.
- **implementation** — `tools/check_vocabulary.py:214-216` adds `root/.planning/**/*.md` to the document set from which `spec_symbols` and `spec_crates` are harvested. Any plan, review, debug note, or audit report becomes vocabulary authority. Live proof: the gate is red **today**, and its single diagnostic (`undocumented specified architectural crate: fava-canary`) originates from `.planning/phases/07.1.1-…/07.1.1-REVIEW.md:169`, a tracked review document that mentions a binary path — not from any specification. `.github/workflows/architecture.yml:20` runs the gate, so CI is failing on prose in a non-authoritative record.
- **observable distinction** — the architecture gate's verdict can be flipped by editing a planning note, in either direction: a stray token turns it red, and adding an unimplemented symbol to a plan satisfies a `spec_symbols` entry that has no code. This report had to be written around the regex to avoid adding a second failure.
- **proposed falsifier** — `tools/tests/test_vocabulary_check.py::test_planning_records_are_not_vocabulary_authority`: fixture writing an unregistered symbol into `.planning/notes.md` and nothing into `docs/spec/`; assert exit 0. Fails today.
- **confidence** — confirmed.

### vocab-tests-cannot-express-the-gap — minor — vocabulary
- **authority** — `AGENTS.md:60`: "Run `python3 tools/check_vocabulary.py` **and its unit tests** for every architectural or public-API change."; brief gate 6 (behavioral proof).
- **implementation** — `tools/tests/test_vocabulary_check.py:158-215`: every one of the nine tests goes through a single helper that writes one crate, one `src/lib.rs`, one `docs/spec/ARCHITECTURE.md`, and a one-term registry. No parameter exists for a second crate, a non-`crates/` package, a file outside `src`, a `.planning` document, or a visibility qualifier. `load_registry`'s ~20 validation branches (`:44-132`), the real-crate diagnostics (`:262-265`), and both reverse-existence diagnostics (`:260`, `:277`) have zero coverage. Only the `struct` keyword is ever exercised; `enum`, `trait`, `type`, and `unsafe` never are.
- **observable distinction** — the test suite passes on a checker that has been blind to `pub(super)` lifecycle owners since M3, and would keep passing if the regex were narrowed further to `struct` alone.
- **proposed falsifier** — the widened-visibility test in `vocab-gate-blind-to-nonpublic-nominals`, plus one negative registry-validation test (`test_rejects_a_term_missing_nearest_nostr`) that would fail if `load_registry`'s Fava-term branch were deleted.
- **confidence** — confirmed.

---

## 6. Proposed checker change and expected yield

**Do not change `PUBLIC_NOUN`; add a second, visibility-aware pattern and keep
the existing one for the spec-document scan** (specifications legitimately write
`pub`-form declarations, and widening `collect_spec_vocabulary` would change
what documents mean).

Concretely, in `tools/check_vocabulary.py`:

1. Add alongside `PUBLIC_NOUN` (line 14) a declaration pattern that accepts any
   visibility, capturing the visibility and the indent:

   ```
   NOMINAL_NOUN = re.compile(
       r"^(?P<indent>[ \t]*)"
       r"(?P<vis>pub(?:\s*\([^)]*\))?\s+)?"
       r"(?:default\s+)?(?:unsafe\s+)?"
       r"(?P<kind>struct|enum|trait|type|union)\s+"
       r"(?P<name>[A-Z][A-Za-z0-9_]*)",
       re.MULTILINE,
   )
   ```

   Note `union` is added; it is currently missing from the alternation.

2. Suppress associated items. A `type` (or any) match must be discarded when it
   sits inside an `impl` block. The cheap correct rule for this codebase: walk
   the file line by line tracking brace depth, and skip any match whose
   innermost open block was introduced by a line matching `^\s*(?:unsafe\s+)?impl\b`.
   A verified-equivalent shortcut on the current tree is to skip `type` matches
   with non-zero indentation — all nine associated items are indented and all
   seven module-level aliases are at column 0 — but the brace-depth rule is what
   should ship, because module nesting would break the shortcut.

3. In `collect_public_symbols` (`:141-165`), use `NOMINAL_NOUN` and record the
   visibility with the symbol so the diagnostic can say which class it is, e.g.
   `undocumented architectural symbol: fava::OpenedRelay (pub(super) struct)`.

4. Widen the walk root: replace `crates_root.glob("*/Cargo.toml")` (`:150`) with
   the workspace members read from the root `Cargo.toml` plus any manifest found
   by `root.rglob("Cargo.toml")` excluding `target/`, and walk `src`, `tests`,
   `benches`, and `examples` under each.

5. Extend `closest_registered_noun` (`:231-240`) to consider registered **symbol
   leaf names** and `spec_symbols`, not only term names. Verified effect on the
   current tree: the hint rate over the invisible set rises from 16 to 17 (it
   newly names `Receipt` for `PersistedReceipt`), and the hint quality improves —
   `WebSocketRelaySession` resolves to `RelaySession` instead of the vaguer
   `Session`.

6. Add an explicit, narrow exemption so the gate stays actionable, rather than
   leaving 13 ordinary internal structs to be registered as architecture. Two
   defensible forms: (a) a `[[exempt]]` table in `vocabulary.toml` listing
   `crate::Name` for internal data with a one-line reason, keeping the decision
   reviewable and in the registry; or (b) exempt declarations that are private
   **and** whose file is under a `tests` module or `src/tests/`. Option (a) is
   preferable — it keeps the closed-by-default posture and makes each exemption
   an explicit, greppable decision.

**Expected yield, measured not estimated.**

| Stage | Change | New diagnostics | Of which true violations |
|---|---|---|---|
| 1 | accept `pub(crate)` / `pub(super)` / `pub(in …)` only | **10** | **9** (only `fava_nip02::Sealed` is a false positive) |
| 2 | stage 1 + private declarations, with associated items suppressed | **66** | **53** (9 lifecycle owners, 44 wrappers/synonyms); the other 13 are the plain-data list in section 2.3 and motivate step 6 |
| 3 | stage 2 + widened walk root (step 4) | **+21** public nominal declarations and **+3** package names | all 24 currently unreviewed; at minimum `NullEventCache` and the three package names need decisions |

Recommended sequencing: ship stage 1 immediately — it is 10 diagnostics, 9 of
them real, and it closes the exact hole that let `OpenedRelay` and its eight
siblings through. Ship step 5 with it (pure improvement, no new failures). Ship
stage 2 together with the step 6 exemption table in one focused vocabulary
change. Ship stage 3 last, since it also forces a decision about whether
`apps/canary` and `falsifiers/` are architecture or scaffolding.

Independently and immediately: the gate is red on `main` today because of a
tracked planning note (`vocab-planning-md-is-authority`), so `.github/workflows/architecture.yml:20`
is failing for a reason unrelated to any code. That must be resolved before any
of the above, or every stage will land on an already-red gate.

---

## Conforming (verified, not merely unexamined)

- **Symbol round-trip is exact in both directions.** 135 declarations matched by
  `PUBLIC_NOUN` under `crates/*/src`; 135 registered symbols; set difference is
  empty both ways (verified by running `collect_public_symbols` against
  `load_registry`). No public symbol is undocumented and no registered symbol is
  dead.
- **Crate round-trip is exact in both directions.** 37 packages under
  `crates/*/Cargo.toml`; 37 registered crates; set difference empty both ways.
- **Registry structural validity.** All 61 terms pass `load_registry`: version is
  1, no duplicate term names, no duplicate registered symbol or crate, every term
  has all six required fields, every `nostr` term has `protocol`, every `fava`
  term has both `nearest_nostr` and `distinction`. Zero validation problems
  emitted.
- **No public nominal declaration exists in test scope** inside `crates/*/src`
  (searched every `src/tests/**`, `tests.rs`, and every `#[cfg(test)]` module for
  `pub struct|enum|trait|type`; zero hits). So the gate's inability to
  distinguish test from production code inside `src` is not currently exploited.
- **No `union`, no `macro_rules!`, and no `pub use … as …` rename** anywhere
  under `crates/*/src` (searched all three). Categories H, I, and the aliasing
  form of G are latent holes with zero current instances.
- **The nine `type` matches I excluded really are associated items**, each read
  in context: eight `IntoIter` bindings inside `impl IntoContactAuthors for …`
  blocks (`crates/fava-nip02/src/query.rs:28-84`) and one `Item` inside
  `impl Iterator for PanicAfter` (`crates/fava-simple-groups/src/tests/saved.rs:105`).
  They are not declarations and must not be counted as vocabulary.
- **`is_structural_crate_metadata` behaves as specified.** Its five suppression
  rules and their five paired controls all hold, and the one live diagnostic on
  the tree (`fava-canary`) is a genuine non-suppressed reference, not a bug in
  the suppression logic.
- **The nine bare Nostr terms are not stale.** `Event`, `EventId`, `PublicKey`,
  `Kind`, `Tag`, `Filter`, `RelayUrl`, `Subscription`, `Coordinate` carry no
  symbols or crates because they are externally owned; `source = "nostr"` with
  `protocol` attribution is the correct record for them.

## Open questions

1. **Are test doubles vocabulary?** `AGENTS.md:56` has no test carve-out, so the
   five `fava-observe` doubles and the seven public doubles in
   `crates/fava/tests/support/semantic_write.rs` are covered by the letter of the
   policy. Either the policy gains an explicit exemption or these need entries.
   I classified them as low-severity violations rather than dropping them,
   because the policy as written does not permit dropping them.
2. **Are `apps/canary` and `falsifiers/` architecture?** They contain a competing
   `EventCache` implementation and the canary scenario surface. If they are
   architecture, 21 declarations and 3 package names need registration; if they
   are scaffolding, the registry should say so explicitly rather than relying on
   a directory glob.
3. **Should aspirational `spec_crates` / `spec_symbols` be distinguishable from
   delivered ones?** 24 crates and 16 symbols are approved-but-absent. A
   `status = "specified" | "delivered"` field would let the gate assert the
   difference instead of the current silence, and would make
   `AGENTS.md:59`'s "current model only" enforceable.
4. **Is the `.planning` scan intentional?** If yes, it contradicts the brief's
   authority order and makes plans binding vocabulary. If no, removing
   `:214-216` also removes today's only live diagnostic — which would mean the
   `fava-canary` finding disappears rather than being fixed, so the two decisions
   must be taken together.
5. Should the symbol key carry the module path so the registry can assert
   reachability, rather than treating a `pub` type in a private module as public
   API? No current entry is affected, but nothing checks it.

---

## Addendum — the checker changed under this audit

Everything above was researched against `HEAD = b221203` (`docs(state): record
phase 07.2 context session`). Partway through, the working tree moved to
`HEAD = f5922f3` (`fix: enforce internal vocabulary variants`), which widens
`tools/check_vocabulary.py` to non-public declarations. The report body is left
as written — it is the pre-fix analysis, and the numbers in sections 1, 2, and 6
are the baseline the fix should be measured against. This addendum reviews what
landed.

### What f5922f3 does

- Adds `NOMINAL_NOUN` (`tools/check_vocabulary.py:18-22`) accepting any
  visibility, and a second symbol set `nominal_symbols` collected alongside
  `public_symbols` in `collect_rust_vocabulary` (`:150-181`).
- New diagnostic class `unapproved nominal vocabulary variant` (`:327`) applied
  to `nominal_symbols - public_symbols`, gated by three filters (`:319-329`):
  skip if the name is in `approved_nominal_names(registry)`, skip if
  `len(words(name)) < 2`, and report **only** when `closest_registered_noun`
  returns a match.
- Adds six tests to `tools/tests/test_vocabulary_check.py:35-83` covering all
  four visibilities, an approved variant, an unapproved wrapper, and an
  unrelated private helper.

This closes finding `vocab-gate-blind-to-nonpublic-nominals` in substance. It
currently emits **48 nominal-variant diagnostics**.

### What it still misses, measured against section 2

Eleven of my 53 violations (15 declarations) are not reported:

- **The `len(words(name)) < 2` skip** silences every single-word noun:
  `Group` (`fava-subscriptions-standard/src/lib.rs:101` — the homonym collision
  with the approved `Group` term, finding `vocab-group-homonym`), `Change` in
  three crates, `Operation` in two, `Target`, `Providers`, `Selected<T>`. A
  bare collision with an approved term name is exactly the case a vocabulary
  gate should catch first, and it is the one case the filter removes.
- **The "must embed a registered noun" filter** silences violations whose whole
  point is that they invent a noun rather than qualify one: `KnownLists`,
  `SemanticCustody`, `PersistedSemantic`, `PreparedSemantic`, `FailureFact`.
  Nine unapproved lifecycle owners were the critical finding; `KnownLists` and
  `PreparedSemantic` are two of them and remain invisible.

### New false positives it introduces

Nine, against section 2.3:

- `fava_nip02::IntoIter` — an **associated type** inside `impl … for …` blocks
  (`crates/fava-nip02/src/query.rs:28-84`), not a declaration at all. This is
  precisely the case step 2 of my proposal exists to suppress; `NOMINAL_NOUN`
  has no `impl`-block guard.
- `CacheState`, `StoreState`, `StoreLimits`, `SocketSink`, `SocketStream`,
  `BoundedTargetText`, `HostileTarget` (a test fixture), `ProjectionOutcome`
  (a test helper) — ordinary internal data and test scaffolding. There is still
  no exemption mechanism (my proposal step 6), so the only way to silence these
  is to register them as architecture, which inflates the registry with
  non-concepts.

### Untouched by the fix

Findings `vocab-walk-root-excludes-three-packages`,
`vocab-spec-side-never-checked-against-reality`, `vocab-reexports-invisible`,
and `vocab-planning-md-is-authority` are all unaffected. `union` is still absent
from both alternations. `collect_rust_vocabulary` still globs
`crates/*/Cargo.toml` and walks only `src`.

### `vocab-planning-md-is-authority`, now proven live

The gate's non-symbol diagnostics on the current tree are:

```
- undocumented specified architectural symbol: SubscriptionPlanDiff (existing noun: SubscriptionPlan)
- undocumented specified architectural crate: fava-canary
- undocumented specified architectural crate: fava-ingest-issued
- undocumented specified architectural crate: fava-owned-deadline
- undocumented specified architectural crate: fava-state-is-a-shared-primitive-hub
```

None of these is a crate or a symbol. Their sources:

- `fava-owned-deadline` — a **finding id** in a sibling audit report
  (`.planning/audit/2026-08-23/transport-wire-ingest.md:137,167`)
- `fava-state-is-a-shared-primitive-hub` — a **finding id**
  (`.planning/audit/2026-08-23/query-state-cache.md:651`)
- `fava-ingest-issued` — a fragment of a **Rust comment inside a proposed
  falsifier** (`.planning/audit/2026-08-23/transport-wire-ingest.md:577`)
- `SubscriptionPlanDiff` — a **proposed** API in a code block
  (`.planning/audit/2026-08-23/subscriptions-diagnostics.md:142,152`)
- `fava-canary` — a binary path in a tracked planning review

Four of the five were created by this audit itself, in the last hour, by agents
writing reports as instructed. The architecture gate in
`.github/workflows/architecture.yml:20` is now red because of the audit's own
prose. This is no longer a theoretical hazard: `collect_spec_vocabulary`
(`:214-216`) treating `.planning/**/*.md` as vocabulary authority makes it
impossible to write an architecture finding whose id or falsifier mentions a
`fava-` concept without breaking CI. Severity for `vocab-planning-md-is-authority`
should be raised from minor to **major** on this evidence.
> Historical audit record. Superseded by STATE-ARCH-1; not current implementation guidance.
