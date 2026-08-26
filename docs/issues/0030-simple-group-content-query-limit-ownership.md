# Remove simple-group content limit ownership

**Status:** implemented
**Branch:** `fix/simple-group-content-query-limit`
**Authority:** generic query semantics, PROTO-006
**Approved by:** Pablo, 2026-08-26

## Defect

`SimpleGroup::events` currently refuses an ordinary `Query` without a result
limit and refuses caller-selected limits above 4,096. Generic Fava observation
accepts queries with or without a whole-query limit. Adding exact group context
and host acquisition does not create a distinct result-bound invariant.

This makes a protocol capability override query-owned policy while presenting
itself as a pure transformation of an ordinary query.

## Decision

`SimpleGroup::events` owns only exact lowercase `h` insertion and acquisition
from the group's hosts with ordinary local result visibility. It preserves the
caller's complete ordinary query, including an absent or caller-selected result
limit.

Record, discovery, and snapshot bounds remain unchanged because those helpers
construct complete capability-owned queries or consume externally supplied
collections. This issue adds no public symbol, type, crate, lifecycle, or
compatibility path.

## Executable evidence

- The external crate API accepts `group.events(Query::events())` and retains no
  result limit.
- The public `fava` facade opens that unbounded group query and preserves local
  write visibility.
- A caller-selected result limit, including one above 4,096, survives group
  scoping unchanged.
- Existing exact-`h`, conflicting-context, host acquisition, authority,
  freshness, and ordering evidence remains green.

Before the implementation changed, the named owner test failed at the retained
guard with `simple group content requires an explicit result bound`. Deleting
the guard makes the same test pass. Restoring either the missing-limit refusal
or the 4,096 maximum makes the unbounded or 8,192-limit assertion fail.

## Validation

Green:

- all 33 owner unit tests, 5 architecture tests, 10 external public-API tests,
  and 10 public-facade simple-group tests through Cargo;
- the same four affected targets through Bazel;
- focused `fava-simple-groups` Clippy with warnings denied;
- changed-diff whitespace validation.

The repository-wide rustfmt check remains red because the baseline requires
unrelated formatting changes across untouched files. The vocabulary command
and 2 of 123 vocabulary-tool tests remain red on the pre-existing public
inventory and candidate-coverage backlog. This slice changes no public symbol,
crate, or vocabulary registry entry.
