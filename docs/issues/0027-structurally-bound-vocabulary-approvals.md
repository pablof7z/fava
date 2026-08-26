# 0027 — Vocabulary approvals are not bound to Rust structure

**Status:** implemented; signing hard-paused pending independent acceptance
**Raised:** 2026-08-25, by Pablo

## Problem

Kind-9999 approval content currently binds only the hand-written vocabulary
record. A term can keep that text while its Rust declaration, exported path,
re-export, field, variant, method, signature, or approved private state changes.
The old signature then remains authoritative for structure the signer never
reviewed.

The approval page also offers a bulk signing path and renders a structured
interpretation of the content rather than the exact bytes submitted to the
signer. Neither behavior proves an explicit per-term decision over a visible
payload.

## Resolution

One pinned review snapshot owns both the readable interface and exact Rust
structure attached to every registry and reviewable candidate term. Every
canonical packet now leads with:

- the exact term name;
- its normalized one-line purpose; and
- a complete readable inventory of every bound declaration, public item, and
  re-export, with kind, exact signature, and a description of what it does.

Constructors are called constructors. Fields, variants, methods, functions,
constants, type aliases, bound private declarations, and exported paths remain
individually visible. Curated crate-inventory purposes win when present;
otherwise rustdoc prose is used, including `Errors`, `Panics`, and `Safety`
semantics after examples are removed. An item with neither gets an explicit
signature-derived description rather than disappearing. The generator proves
that every compiler-bound declaration appears exactly once in the readable
inventory.

Edge and error semantics follow the interface. Governance metadata and the
deterministic machine JSON follow those sections as secondary detail. The
signed bytes include all sections; changing either a human description or the
compiler structure makes every previous signature stale.

The exact compiler-derived per-term records contain:

- every exact compiler-rendered public declaration rooted at that term,
  including fields, variants, methods, and signatures;
- every exact public re-export path and its source path; and
- every non-public nominal declaration whose exact name and owning crate are
  classified by that term, including its source path and declaration body.

An explicit empty interface and structural record is bound for a term with no
implemented Rust structure, so later implementation also invalidates prior
authority. A combined per-term review packet above 192 KiB is refused rather
than truncated or made impossible to submit through the bounded endpoint.

`tools/vocabulary_structure.py` recompiles the snapshot with the pinned nightly
rustdoc, `cargo-public-api`, and crate README descriptions. Rust source,
registry/candidate research, human interface descriptions, and extractor inputs
all participate in the input fingerprint. CI rejects any committed snapshot
that differs from fresh output. Approval startup performs the same check, and
the POST boundary refuses input-file drift after startup.

Crates listed by `complete_public_api_crates` additionally require one and only
one vocabulary owner for every compiler-rendered public identity. The first
closed crate is `fava-simple-groups`: its crate-root module, nominal types,
fields, variants, methods, conversion, and all seven free query/edit/materializer
functions are bound. Its semantic-description gate rejects generated
"compiler-visible" restatements and other signature-only tautologies.

The approval page renders the name, purpose, interface, and edge semantics
first. Governance and exact machine content are collapsed secondary detail;
the complete exact signed Markdown remains separately inspectable. No bulk
signing path exists. Signing is additionally locked at the server boundary with
HTTP 423, the UI does not connect a signer, and the pause has no runtime or CLI
override. A later independent acceptance must deliberately change code before
any event can be submitted.

GET review state recomputes the current compiler/documentation input fingerprint
on every request. If it differs from the snapshot, signed terms become `stale`,
unsigned terms become `blocked`, every row names the drift, and the top-level
payload reports `snapshot_inputs_current: false` even though signing is already
hard-paused.

The independently reviewed `fava-simple-groups` signing package has a second,
repository-owned canonical boundary. `tools/vocabulary_package.py` sorts the 22
exact term names by their UTF-8 bytes and serializes each exact canonical
Markdown payload as:

`u64be(name UTF-8 byte length) || name UTF-8 || u64be(Markdown UTF-8 byte length) || Markdown UTF-8`

The package is the concatenation of those records with no prefix, suffix,
separator, terminator, Unicode normalization, or newline conversion. The
unsigned 64-bit big-endian lengths make both fields and every record boundary
unambiguous. The checked-in manifest records the ordered term index and name,
both field lengths, frame length, payload SHA-256, total byte length, and whole
package SHA-256. CI regenerates the package and rejects any byte difference in
the manifest; a stale compiler/documentation input snapshot is rejected before
package construction.

`docs/internals/approvals.jsonl` remains append-only. Introducing structural
content intentionally makes existing text-only events stale without rewriting
or deleting them. A new signature appends beside all earlier events.

## Proof contract

- Changing a bound field, variant, method signature, exported path, re-export,
  or classified private declaration makes snapshot check fail.
- Changing a human interface description makes snapshot check fail and makes
  the previous signed canonical Markdown non-authoritative.
- Refreshing the snapshot after that change makes the old event content fail
  exact canonical-payload matching.
- An unrelated implementation-body change requires snapshot recompilation but
  does not change a term's structural payload.
- Python and Rust render byte-identical complete SimpleGroup approval Markdown.
- The page contains no multi-term signing control, shows the readable signed
  sections first, and retains the exact raw `term.markdown`.
- Every approval POST is refused while independent acceptance is pending.
- Replaying one event is idempotent; signing changed structure appends without
  mutating historical lines.
- The 22 `fava-simple-groups` vocabulary identities are enumerated from the
  registry, bind all 113 compiler-rendered public items exactly once, expose
  147 readable interface items, and have zero review gaps.
- `SimpleGroupStateEventKind` binds Metadata→39000, Admins→39001,
  Members→39002, Roles→39003, LivekitParticipants→39004, and Pins→39005 in the
  term purpose, enum description, individual variants, and conversion item.
- Reordering input rows cannot change canonical package bytes; changing one
  Markdown byte changes both its term hash and the package hash; ambiguous raw
  concatenations remain distinct under length framing; any manifest byte drift
  fails the package check.

## Validation

- `python3 -m unittest tools.tests.test_vocabulary_structure tools.tests.test_vocabulary_approval`
- `python3 -m unittest tools.tests.test_vocabulary_check tools.tests.test_crate_readme_api`
- `python3 tools/vocabulary_structure.py check`
- `python3 -m unittest tools.tests.test_vocabulary_package`
- `python3 tools/vocabulary_package.py check`
- `python3 tools/check_vocabulary.py`
- `cargo test -p fava --test vocabulary_governance` excluding the intentionally
  red all-terms approval and independent terminal-name gates

The combined SimpleGroup tree closes the earlier candidate-research mismatches.
The all-terms-approved repository gate remains red because owner signatures are
external; this issue does not approve, replace, or hide that backlog.
