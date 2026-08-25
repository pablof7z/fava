# Fava-observe Bazel dependency closure

## Defect

`fava-observe` imports ten first-party production crates and declares all ten
to Cargo, but its Bazel library declares only `fava-query`. Strict Bazel Rust
dependency checking therefore stops composition targets at `fava-observe`
before their tests can execute.

Declaring those direct edges exposes one prerequisite gap: `fava-runtime`
imports the neutral `OperationGeneration` owner from `fava-query`, while its
Bazel library and source-based unit-test targets declare no corresponding
first-party edge.

The dependent `fava` facade likewise imports and re-exports `fava-runtime` but
omits that Cargo normal dependency from its Bazel library target. This is the
last direct declaration required before the blocked Redb process-kill
composition can compile.

## Scope

- Map every existing `fava-observe` production import to its direct first-party
  Bazel library dependency.
- Add executable evidence that production imports, Cargo normal dependencies,
  and Bazel library edges remain equal.
- Declare the prerequisite `fava-runtime -> fava-query` edge on the two
  existing runtime targets that compile runtime sources directly.
- Declare the existing facade's direct `fava -> fava-runtime` edge so the
  dependent process-kill composition can execute.
- Change no Cargo dependency, Rust behavior, public API, ownership, or
  architectural vocabulary.

## Falsifier

Remove one mapped Bazel edge while leaving the Rust import and Cargo dependency
intact. The dependency-mapping test must name the missing edge, and a dependent
composition target must fail to build under Bazel's undeclared-import checks.

## Exit gates

- The dependency-mapping evidence passes through Cargo and Bazel.
- `//crates/fava-observe:lib` builds.
- The previously blocked Redb composition tests execute without undeclared
  first-party imports.
- Cargo tests for the declaration owners pass; the affected Redb composition
  runs to its independent behavioral disposition.
- Focused formatting and diff checks pass.

## Executable evidence

Before the declarations changed, the new mapping test failed through both
Cargo and Bazel: production source and Cargo named ten first-party crates while
the Bazel library named only `fava-query`. The dependent Redb Bazel invocation
failed to build `fava-observe` with 76 unresolved first-party references and
executed none of its tests.

After the complete mapping was green, a deliberate removal of only
`//crates/fava-ingest:lib` caused both copies of the mapping test to report the
exact nine-versus-ten set difference. The Bazel library independently failed
at `src/ingest.rs:73` on unresolved `fava_ingest`; the other nine mappings were
unchanged. Restoring that one line returned the manifest to SHA-256
`16ba8018d68be41e3c6b1e26b97a24307ff5122de8db8050216d05ecf7b25743`
and returned Cargo and Bazel evidence to green. No mutation path remains.

## Outcome

The existing `fava-observe` library now declares exactly the ten first-party
normal dependencies named by its production imports and Cargo manifest. The
mapping test derives all three sets independently and rejects missing or extra
edges. The two prerequisite declarations discovered by the fresh composition
build are also explicit: `fava-runtime -> fava-query` for its library and
source-based unit tests, and `fava -> fava-runtime` for the facade library.

Cargo behavior and architecture ownership are unchanged: no Cargo manifest or
production Rust source changed, and every new Bazel edge points to the existing
Cargo owner.

## Validation disposition

Green:

- all 28 `fava-observe` Cargo tests;
- all 37 `fava-runtime` Cargo tests;
- all eight public local-source composition Cargo tests;
- the Cargo and Bazel dependency-mapping evidence;
- focused Clippy with warnings denied, focused rustfmt, and diff checks;
- the `fava-observe` Bazel library build;
- all four `fava-runtime` Bazel test targets; and
- the Redb Bazel process-kill composition, seven tests.

The second formerly blocked Redb target now builds and executes under Bazel,
but exposes an unchanged behavioral failure also reproduced through Cargo:
`redb_initial_route_idempotence_compares_complete_persisted_effect` reports
`shortfall mismatch was accepted as idempotent`; its other 21 tests pass. This
slice does not change Redb behavior. Successful Bazel target results are
followed by the managed macOS sandbox's known exit-37 `sysctl` shutdown error;
the target dispositions above precede that launcher failure.

Repository-wide `cargo fmt --all --check` remains red on unchanged
simple-groups and vocabulary-governance formatting drift. The new Rust
evidence file passes rustfmt independently.

Vocabulary validation remains independently red on the repository's existing
inventory backlog. Its focused Python suite ran 123 tests: 121 passed and the
two existing researched-candidate coverage tests failed. This build-only slice
adds no public symbol, crate, or vocabulary term.
