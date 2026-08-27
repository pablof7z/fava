# Simple-group public API has one canonical spelling

**Status:** implemented
**Approved by:** Pablo, 2026-08-27
**Branch:** `refactor/simple-group-api-cleanup`
**Authority:** PROTO-006, public API governance

## Decision

`SimpleGroup::new(id, relays: Vec<RelayUrl>)` is the sole constructor for a
reusable simple-group value. It returns
`Result<SimpleGroup, SimpleGroupConstructionError>` and retains the construction
invariants and normalization owned by `SimpleGroup`.

The crate exposes its public items at `fava_simple_groups::*`. Source modules,
including `management`, are private implementation organization. Each public
item therefore has one canonical path; source-file layout is not public API.

No constructor alias, public implementation module, deprecation, shim, or
compatibility path exists.

## Ownership and forcing requirement

The ordinary constructor owns formation of the central `SimpleGroup` value, so
its conventional Rust name is `new`. A `from_*` name would imply conversion
from one distinguished source representation even though construction equally
requires the group id and relay selection.

The crate root owns its public namespace. A source module becomes public only
when an approved user-facing subsystem requires a stable namespace; the
current files group implementation responsibilities and do not define such
subsystems.

## Executable falsifiers

- External code constructs a group only through `SimpleGroup::new(id, relays)`.
- Empty ids and empty relay vectors retain their exact typed refusals.
- The compiler-visible API contains exactly one `SimpleGroup` constructor.
- The compiler-visible API contains no public `management` module or nested
  management-item paths.
- All management values and functions remain available at the crate root.
- Vocabulary, specifications, examples, generated API inventory, Rust tests,
  and canary code contain no obsolete simple-group constructor spelling.

## Validation

Green:

- all 35 owner unit tests, 6 architecture tests, 6 external Cargo tests, 43
  runnable doctests, and 2 compile-fail constructor tests;
- all 4 public-facade simple-group Cargo tests and the canary library build;
- all 4 `fava-simple-groups` Bazel targets;
- compiler-visible API audit proving `SimpleGroup::new` and crate-root
  management functions with no public `management` path;
- compiler-derived vocabulary structure, canonical 34-term package, all 30
  focused structure/package unit tests, and the complete README inventory;
- exact obsolete-token search and changed-diff whitespace validation.

Repository baseline failures remain outside this issue: the public-facade Bazel
target lacks already-imported cache dependencies, the canary binary contains a
committed merge marker, Clippy reaches an existing `people.rs` `op_ref` lint,
the repository-wide rustfmt check reports unrelated files, and the global
vocabulary registry retains its documented cross-repository review backlog.
