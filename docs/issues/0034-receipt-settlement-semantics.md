# Distinguish terminal receipts from acknowledged publication

**Status:** implemented
**Approved by:** Pablo, 2026-08-27
**Branch:** `fix/receipt-semantics`
**Authority:** WRITE-002, receipt ownership, public API governance

## Defect

The application predicate named `all` accepts after every currently desired
destination reaches any terminal fact. A receipt containing only rejections is
therefore accepted by a name that does not tell callers whether it means
completion or publication success.

Aggregate acknowledgement counts are also insufficient for complete success.
Receipt evidence retains facts for withdrawn destinations, so a historical
acknowledgement can make the count equal the smaller current desired set while
a currently desired destination is rejected.

## Decision

The facade exposes two exact predicates:

- `all_terminal()` accepts when routing is settled and every currently desired
  destination has an exact terminal fact. Rejection, exhausted delivery, and
  ambiguous handoff satisfy it.
- `all_acknowledged()` accepts when routing is settled, the current desired set
  is nonempty, and every destination in that exact set has acknowledgement
  evidence.

The predicates read the receipt owned by the publication lifecycle. They own no
state, route selection, destination evidence, retry policy, or receipt outcome.
The former `all` spelling is absent; there is no alias, deprecation, shim, or
compatibility path.

## Executable falsifiers

- A mixed acknowledged, rejected, and ambiguous terminal receipt satisfies
  `all_terminal()` and does not satisfy `all_acknowledged()`.
- An all-acknowledged nonempty current desired set satisfies both predicates.
- `Write::settled(all_acknowledged())` remains pending after only a subset of
  the current destinations acknowledge and returns after the final one does.
- An empty settled route satisfies terminal completion but not acknowledgement.
- An unsettled route and a desired destination without a fact satisfy neither.
- A historical acknowledgement for a withdrawn destination cannot mask a
  current desired rejection, even when the aggregate acknowledgement count is
  greater than the current desired count.
- `Write::settled(all_acknowledged())` returns `PublishError::NotReached` with
  the complete receipt when terminal mixed evidence makes acknowledgement
  impossible.
- Compiler-visible API evidence, vocabulary, specifications, examples, tests,
  and canaries contain no public `fava::all` symbol or `settled(all())` call.

## Validation

Green:

- all 9 focused public receipt-settlement tests, including exact current-route,
  empty-route, missing-fact, withdrawn-history, and `NotReached` falsifiers;
- every non-governance `fava` test reached before the repository approval gate;
- the standalone canary library compilation and all 3 `fava-nip02` doctests;
- the generated `fava` README public-API inventory, current compiler-derived
  vocabulary structure, and all 79 focused vocabulary and README tool tests;
- exact obsolete-symbol and changed-diff whitespace searches.

Repository baseline blockers remain outside this issue. The global vocabulary
approval test retains its unsigned and human-description backlog, strict Clippy
stops in the existing `fava-fetch-cache` `map_unwrap_or` findings, and
repository-wide rustfmt reports unrelated files.
