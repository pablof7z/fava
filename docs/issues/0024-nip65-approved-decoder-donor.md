# 0024 — Isolated NIP-65 decoder donor

**Status:** separately approved donor; intentionally absent from state-foundation
**Branch:** `nip65-approved-decoder`
**Worktree:** `/private/tmp/fava-nip65-approved-decoder`
**Evidence owner:**
`docs/issues/0023-nip65-approved-decoder-evidence.md` in that worktree

## Boundary

The decoder pilot owns tolerant malformed-URL handling, present-empty marker
handling, distinct-result bound accounting, the named `WrongKind { actual }`
error surface, its README/API inventory, and its focused Cargo/Bazel/mutant
evidence. None of those changes are part of state-foundation.

State-foundation retains only changes forced by its approved ownership model:

- `RelayUrl` comes from `nostr`, because `fava-state` no longer owns relay
  identity exports;
- NIP-65 event winner selection is an ordinary bounded `Query`, so
  `relay_lists` replaces protocol-local source identity, timestamp, and
  `supersedes` state;
- the crate participates in exhaustive comparator-source inventory so a local
  timestamp/id winner cannot return unnoticed;
- router callers consume already-selected query records instead of maintaining
  a second `KnownLists` winner lifecycle.

The state worktree deliberately keeps the pre-pilot decoder behavior for
malformed URLs, empty markers, `WrongKind(u16)`, and `InvalidRelay(String)`.
Landing the donor remains an independent operation against the post-foundation
crate surface and must carry its own causal evidence.
