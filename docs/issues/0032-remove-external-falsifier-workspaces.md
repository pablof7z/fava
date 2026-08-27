# 0032 — Remove external falsifier workspaces

**Status:** complete
**Branch:** `architecture/remove-external-falsifiers`

## Problem

The repository carries two standalone downstream Cargo workspaces solely to
prove that public provider and semantic-write contracts can be implemented
outside the main workspace. They duplicate behavior already exercised by
workspace conformance and public-API tests, add separate lockfiles, and require
dedicated CI and tooling paths.

## Resolution

- Delete `falsifiers/`.
- Remove its dedicated CI jobs, Bazel discovery, BDD mappings, and structural
  scan paths.
- Keep provider contracts public and default implementations on those
  contracts, but remove the requirement for alternative or out-of-workspace
  implementations.
- Keep ordinary conformance suites, public-API tests, deliberate-break tests,
  dependency-negative tests, and application canaries.

## Validation

- PASS: semantic-write/state-foundation tooling, 19/20 deletion-relevant and
  unchanged tests; the remaining catalog-count assertion also fails on `main`
  after the concurrent simple-groups update (`expected 100`, current `118`).
- PASS: canary tooling, 9/9.
- PASS: focused `fava-subscriptions` strict Clippy with `--no-deps`.
- PASS: comparator analyzer fixtures, 7/7; the repository-discovery case remains
  blocked by the inherited parse error in `apps/canary/src/main.rs`.
- PASS: root Bazel `state_comparator_sources` target and stale-reference search.
- PASS: JSON/JSONL parsing and `git diff --check`.
- BASELINE FAILURE: workspace tests pass until the two existing vocabulary
  backlog tests and, with those skipped, the malformed canary source.
- BASELINE FAILURE: workspace strict Clippy stops in unchanged `fava-fetch-cache`
  and `fava-query` code.
- BASELINE FAILURE: `cargo fmt --all -- --check`, vocabulary approval/structure,
  canary compilation, and full Bazel all fail in unchanged repository surfaces.
