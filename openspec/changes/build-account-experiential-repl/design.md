## Context

The authoritative model already says a session contains accounts and current-account selection (`ID-001`), current account is a reactive query input (`ID-002`), missing current identity refuses before write acceptance (`ID-003`), and accepted authorship does not follow later switches (`WRITE-003`). The Rust partial query spec names `CurrentAccount::pubkey()` as a reactive root whose open query reroots automatically. Current implementation exposes exact-key signer attachment generations but no public current-account owner or reactive value binding.

`examples/simple-groups` owns selected account as app state and passes the author explicitly. That was sufficient for its group workflows but cannot prove the required current-account convenience or reactive query behavior without duplicating Fava responsibilities.

## Goals / Non-Goals

**Goals:**

- Build a minimal `examples/account` app whose entire purpose is account lifecycle and its immediate effects on writes and open queries.
- Force the authoritative current-account and `$currentPubkey` contracts through Fava’s public API.
- Prove accepted-write author stability, automatic query recompilation/rerouting, and stale-generation isolation.
- Reuse the proven shell grammar and extract terminal mechanics only where this second consumer needs them.

**Non-Goals:**

- Profiles, contacts, relay preferences, identifiers, relay information, bookmarks, lists, messaging, encryption, or arbitrary account-owned protocol state.
- Manual application propagation of account changes into writes or queries.
- A general reactive-value algebra beyond the current-account public root needed by this slice.
- Persistent session restore unless implementation of the required runtime owner makes it inseparable from the focused lifecycle.

## Decisions

### Fava session owns current selection

The runtime session remains the authority for attached signers and pubkey-only accounts and gains the one optional current-account selection required by `ID-001`. Selection changes advance a bounded monotonic session revision and emit one coalescible change signal. Removing the selected account clears selection atomically. Replacing a signer for the same key advances signer generation without changing account identity.

The application owns only aliases that resolve user input to public keys. It does not own a second selected-public-key fact.

Alternative: retain selected account in `e2e-support` and call Fava with explicit authors. Rejected because it cannot satisfy automatic current-account writes or query dependencies and creates two authorities.

### Current-account writes resolve before acceptance

The facade exposes one current-account publication scope or equivalent convenience selected through focused vocabulary review. It snapshots the selected public key synchronously, refuses before creating a write or receipt when selection is empty, and lowers to the ordinary accepted-write path with that exact author. Signer lookup still uses exact attachment generation during signing. Later selection changes never mutate accepted work.

Alternative: the app reads current key and calls `.by(key)`. Rejected because it is precisely the author-threading DX failure under test.

### `$currentPubkey` is one first-class reactive root

The facade/query surface exposes the current public key as a reactive value accepted by author and tag-value filter axes. Empty selection is an empty set and therefore matches nothing; it never removes or widens the filter. This slice implements only the current-account root and the minimum binding needed by query filters, not general query-derived `ValueSet` algebra.

An observation retains one stable application handle. Session revision changes trigger dependency recompilation inside the query/observation owner, which computes the new concrete query, updates route demand and relay subscriptions, re-evaluates cache/write-store sources, and publishes a new immutable snapshot. The app never closes or reopens it.

Alternative: have the app subscribe to session changes and rebuild `Query`. Rejected because `ID-002` assigns that work to Fava.

### Exact revision identity isolates late completions

Each compiled account-dependent query generation carries the session revision and operation generation that produced it. Relay results, route changes, subscription events, and local-source completions apply only to the current exact generation. A switch retires old live demand while preserving cached public events and already accepted writes/receipts.

Signer replacement uses existing exact attachment generation. Invocation begun under a retired generation cannot become a current signing completion; writes already accepted for that author remain inspectable and settle according to their own lifecycle.

### The app uses deliberately plain test events

`examples/account` publishes an explicit caller-supplied kind and content through the current-account convenience, lists receipts, and opens bounded event queries authored by `$currentPubkey`. This avoids importing unrelated protocol semantics while still proving author resolution, publication, query recompilation, routing, and diagnostics.

### Live proof observes the same handle and independent relay truth

The scenario creates/imports A and B, publishes through A, opens one `$currentPubkey` observation, switches to B, publishes through B, clears/removes selection, and switches rapidly around delayed completions. Typed app output identifies the stable observation and each current generation. The harness independently reads exact events and authors through ordinary relay `REQ`/matching `EOSE`; it never constructs or publishes action events.

### DX evidence blocks completion

The app is audited for explicit author threading, session-change listeners, query rebuilding, observation reopening, subscription mutation, route recomputation, and generation filtering. Any occurrence outside presentation/test assertions is a public Fava defect and blocks completion.

## Risks / Trade-offs

- **Reactive query support reaches several core owners** → Land one vertical current-account slice with exact cross-owner evidence; do not generalize to all reactive values.
- **Current selection and signer availability are distinct** → Model pubkey-only and signer-backed accounts explicitly; selection can support reads while writes report the existing typed signer outcome.
- **Rapid switches can expose stale work** → Carry exact session revision plus operation generation through every late-completion boundary and test deliberate delays.
- **Second-consumer extraction can over-generalize** → Share only identical shell/terminal mechanics; keep account commands and result DTOs local.
- **Ordinary relays may reorder frames** → Prove current snapshots through generation identity and matching `EOSE`, not arrival timing assumptions.

## Migration Plan

1. Approve the focused current-account/reactive vocabulary and executable ownership falsifiers.
2. Land session current-selection behavior, then current-account write resolution, then `$currentPubkey` query binding as independently reviewed focused slices.
3. Extract only shell/terminal mechanics forced by the second app and keep simple-groups replay bytes stable.
4. Build the account commands, deterministic scenario, delayed-completion falsifiers, and live relay proof.
5. Review, rebase, validate, and merge every slice and the final app to `main`. No persisted production schema or compatibility path is introduced.
