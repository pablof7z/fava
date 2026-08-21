# Fava write-bounds Bazel dependency edge

**Status:** complete
**Branch:** `milestone/m7-semantic-writes`

## Problem

`//crates/fava:write_bounds` imports `fava_routing` directly but its Bazel
target does not declare `//crates/fava-routing:lib`. Cargo succeeds because the
crate is already a development dependency, while strict Bazel dependency
checking makes `bazel test //...` fail before the M7 full gate can run.

The omission predates Phase 7 (`c58bf22:crates/fava/BUILD.bazel`).

## Scope

- Add the missing direct dependency to the existing `write_bounds` test target.
- Change no Rust source, public API, architectural vocabulary, or behavior.

## Exit gates

- `bazel test //crates/fava:write_bounds` passes.
- The target's dependency list names every directly imported first-party crate.
- `git diff --check` passes.

## Outcome

- Declared the existing direct `fava-routing` test dependency.
- Restored the full Bazel graph as a usable M7 verification surface.
