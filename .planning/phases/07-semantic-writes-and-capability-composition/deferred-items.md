# Phase 07 Deferred Items

## Pre-existing Bazel metadata defect

- `bazel test //...` reaches `//crates/fava:write_bounds` and fails because that target imports `fava_routing` without declaring `//crates/fava-routing:lib`.
- The omission predates Plan 07-03: `git show c58bf22:crates/fava/BUILD.bazel` shows the `write_bounds` target without the direct edge.
- Plan 07-03 did not alter that target. Its owned target, `//crates/fava:semantic_write_publication`, passes.

