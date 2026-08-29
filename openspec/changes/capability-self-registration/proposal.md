## Why

`fava-simple-groups` knows what kind 10009 means and owns everything else about it — its queries, its typed edits, its decoders. It should also own the claim that it handles 10009, and say so without the application repeating it.

On `origin/main` the layering is already most of the way there. The facade ships no applier, has no real dependency on any protocol crate (the three appear only as dev-dependencies), and its registration door is already neutral — `applier` / `appliers`, taking anything implementing the contract. Each protocol crate already owns and publicly exports its own applier factory. What remains wrong is narrower than it first appears:

- The exported factory hands a consumer the low-level contract. A crate that should only construct typed edits exposes `applier() -> Arc<dyn EditApplier>` in its public surface.
- Every application must repeat, at assembly, a claim its `Cargo.toml` already made. Depending on `fava-simple-groups` and then having to say "and please handle 10009" is the same fact stated twice.

This supersedes issue 0051, which decided the opposite: that a protocol-crate consumer must have no compiler-visible applier factory, and that a separate implementation crate is the necessary seam because Rust requires a cross-crate factory to be public. The premise is right and the conclusion does not follow — what must be public is an *opaque capability*, which exposes no low-level term to the consumer, and that needs no separate crate. 0051's own counterexample, `fava_simple_groups::__fava::materializer`, was objectionable because it published the contract, not because it lived in the protocol crate.

Note that `origin/main` is closer to this target than the `fix/private-builtin-materializers` branch, which moved away from it by making the facade own three shipped appliers and take real protocol dependencies. This change builds on `origin/main`.

## What Changes

- **BREAKING** New opaque `Capability` type in `fava-write`, wrapping one `EditApplier`. A consumer holding one can learn neither the claimed kind nor the applier.
- **BREAKING** New `fava_write::register_capability!` macro. A protocol crate invokes it once; linking the crate is the registration. The registry lives in `fava-write` because every protocol crate already depends on it — putting it in `fava` would make protocol crates depend on the facade and invert the layering.
- **BREAKING** `fava-simple-groups`, `fava-nip02`, and `fava-bookmarks` each replace their public `applier()` factory with a capability declaration. The applier implementations stay where they already are; only the way the claim leaves the crate changes.
- **BREAKING** The builder's `applier` / `appliers` methods become one `capability` door taking a `Capability`. Application-defined edit semantics wrap their own applier with `Capability::from_applier`, so there is one door and one vocabulary.
- Capabilities collected from the link-time registry and capabilities named explicitly at assembly land in the same index, under the existing duplicate-kind and 64-claim refusals. Nothing overrides a claim.
- Publishing an edit whose kind no capability claims stays the existing refusal — now the signal that a protocol crate is not in the dependency graph.

## Capabilities

### New Capabilities
- `write/edit-capabilities`: what a capability is, how a protocol crate declares its claim on a kind, and what an application can and cannot learn from one.
- `publication/capability-registration`: how declared capabilities reach the facade's kind index, how explicit and self-registered ones combine, and how conflicts and absences are refused.

### Modified Capabilities

None yet in `openspec/specs/`; the `write/edit-vocabulary` capability from `plain-edit-vocabulary` is a sibling, not a parent.

## Impact

- `crates/fava-write` — gains `Capability`, the registry, and `register_capability!`, plus one new external dependency for link-time collection (`inventory` or `linkme`, chosen on target support).
- `crates/fava/src/builder.rs` — the `applier` / `appliers` methods become `capability`, and the registry is read at assembly before recovery.
- `crates/fava-simple-groups`, `crates/fava-nip02`, `crates/fava-bookmarks` — each drops its public applier factory and declares a capability instead. Their architecture tests' export allow-lists change accordingly.
- `crates/fava/Cargo.toml` — unchanged. It already depends on the three protocol crates only as dev-dependencies.
- No `fava-builtin-codecs` deletion is required: that crate exists only in the `fix/private-builtin-materializers` working tree and on no branch.
- `docs/issues/0051-built-in-protocol-materializers.md` — superseded, with the reversal and its reasoning recorded rather than the file being rewritten in place.
- Depends on `plain-edit-vocabulary`; this proposal is written in that change's vocabulary (`EditApplier`, `apply`, `EventEdit`, `RevisionId`).
