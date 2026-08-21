# `fava-simple-groups` multi-relay NIP-29 North Star

**Status:** approved
**Approved by:** Pablo, 2026-08-21
**Owning phase:** 07.1.1

## Scope

Promote the user-authored `fava-simple-groups` whiteboard README into the
repository's product, architecture, vocabulary, and delivery authorities before
implementation. This focused architecture slice changes no runtime behavior.

The README at `crates/fava-simple-groups/README.md` is the app-facing North
Star. Exact Rust signatures remain implementation targets until executable
public evidence makes them real, but plans and implementation must preserve its
observable model rather than shrinking the model to fit current code.

## Approved model

`Group` is a pure value containing one opaque NIP-29 group id and a non-empty,
bounded set of host relays.

- One host is the ordinary case.
- Several hosts let an application treat relay-local forks as one app-facing
  feed without claiming they are one relay authority.
- Content reads add the exact `h` value and ask the complete host set.
- Relay-authored records add the exact `d` value and retain per-host authority.
- Events deduplicate by event id; record disagreement remains visible per host.
- Writes use the exact complete host set as `WriteRouting::Explicit` and never
  invoke automatic routers.
- The application chooses a side by constructing a single-host `Group`; the
  capability never silently chooses a canonical relay or migration.
- Helpers return ordinary `Query`, `WriteIntent`, event, or
  `ReplaceableEventEdit` values. The crate owns no socket, observation,
  signing, store, delivery, retry, or receipt lifecycle.
- Publication is kind-blind. A custom event kind can use the same group path.

## Vocabulary approval

### Crate: `fava-simple-groups`

- **Closest existing concept:** the NIP-29 event-kind protocol capability.
- **Observable distinction:** the capability is named for the simple-groups
  product concept and exposes one multi-relay app aggregation while preserving
  relay-local NIP-29 authority.
- **Counterexample:** naming or implementing only a single-relay wrapper cannot
  express the README's forked `photos` feed and visible per-host disagreement.
- **Owner and lifecycle:** pure protocol values and transformations owned by
  `fava-simple-groups`; all work lifecycles remain with Fava's existing query
  and publication owners.
- **Forcing requirement:** GROUP-01 through GROUP-12 and PROTO-006.
- **Why existing state is insufficient:** no current crate owns typed NIP-29
  group values, record parsing, discovery queries, or exact group-context
  writes; the old specified crate name also contradicts the approved public
  name.
- **Executable falsifier:** an ordinary external app must use only the public
  crate plus public Fava facade to combine two relay-local forks, observe exact
  per-host disagreement, and publish through an exact explicit host set. A
  single-host collapse or universal NIP-29 branch must make the canary fail.

### Public nominal types

| Type | Closest concept | Exact Fava-owned distinction | Owner/lifecycle | Falsifier |
|---|---|---|---|---|
| `Group` | NIP-29 relay-based group | One id over an app-selected non-empty host set; aggregates but does not merge relay authority. | Pure value in `fava-simple-groups`. | Two hosts cannot remain independently attributable. |
| `GroupRecords` | NIP-29 kinds 39000–39005 | Bounded typed selector for relay-authored group records. | Pure query input. | Caller must construct raw kind/tag filters. |
| `GroupSnapshot` | NIP-29 relay records | Pure projection with merged app view, per-host values, and exact disagreement. | Derived from `QuerySnapshot`; no observation lifecycle. | Conflicting metadata is silently field-merged or one host wins invisibly. |
| `SimpleGroups` | NIP-29 discovery and saved-list operations | Namespace for ordinary discovery queries and semantic saved-list edits. | Pure constructors only. | Helper opens work or owns observation state. |
| `GroupMetadata`, `GroupAdmins`, `GroupMembers`, `GroupRoles`, `GroupParticipants`, `GroupPins` | NIP-29 kinds 39000–39005 | Typed bounded parsing of each exact relay-authored record kind. | Pure parsed values. | App must decode raw tags or malformed records become valid values. |
| `PinnedItem` | NIP-29 pin record item | Ordered typed `e`/`a` pin entry without presentation policy. | Pure parsed value. | Pin order or target kind is lost. |
| `SavedGroup`, `SavedRelay` | NIP-29 kind-10009 rows | Typed group and relay-in-use rows retaining author and host evidence through projection. | Pure parsed values. | Same id at two relays collapses or saving authors disappear. |
| `GroupError` | NIP-29 refusal | Typed construction, parsing, bounds, and contradictory-group-context refusal. | Pure error value; no retry policy. | Empty/oversized hosts or malformed group rows are silently accepted or truncated. |

No separate `RelayScope`, group observation, group receipt, group runtime, or
protocol-specific query engine is approved.

## Architectural gates

1. **Ownership:** relay-local record authority stays in exact relay evidence;
   `Group` owns only the app-selected aggregation description.
2. **Dependency direction:** `fava-simple-groups` uses `fava-query`,
   `fava-state`, and `fava-write`; universal owners never depend on it.
3. **Replaceability:** adding/removing the crate changes only application
   selection and artifact metadata.
4. **Failure isolation:** malformed records and one-host failure remain scoped
   and attributable; no helper creates work outside ordinary Fava operations.
5. **Boundedness:** host sets, ids, tags, decoded rows, projections, and
   discovery values have explicit bounds or typed refusal/shortfall.
6. **Behavioral proof:** pure parser/construction tests plus a public two-relay
   canary prove read, fork, write, cancellation, evidence, and close behavior.

## Delivery boundary

This slice promotes the target and schedules implementation. It intentionally
does not add a Cargo package, placeholder Rust API, or empty implementation.
The README-only directory is an implementation anchor, not a claim that the
crate already ships.

## Validation

- `python3 tools/check_vocabulary.py`
- `python3 -m unittest tools.tests.test_vocabulary_check`
- `python3 tools/check_doc_links.py` when available
- `git diff --check`
- repository search proving every authoritative group-capability reference uses `fava-simple-groups`
