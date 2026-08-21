# Delivery identity and budget are separate public facts

**Status:** in progress  
**Requirements:** `HARD-05`, `HARD-06`, `HARD-07`  
**Authority:** `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`,
`docs/spec/ARCHITECTURE.md`, and
`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`

## Problem

Commit `197c278` added the public delivery regression and the publication/store
seams that retry an unreachable destination. Its public target is not
self-contained at committed `HEAD`: the neutral unreachable outcome, publisher
mapping, standard policy, receipt validation, and independent spent-attempt
facts still exist only in five intentional dirty files.

The required distinctions are exact:

- a failed connection establishes `Unreachable` and spends no attempt;
- a definite pre-handoff refusal spends exactly one attempt;
- a full `EVENT` handoff without relay `OK` remains ambiguous;
- monotonic attempt generation identifies late completions, while
  `spent_attempts` alone feeds the delivery ceiling.

## Adopted WIP fingerprints

The plan adopts these current bytes exactly once. The fingerprints were taken
before staging or editing any of the five paths:

| Path | SHA-256 |
|---|---|
| `crates/fava-delivery-standard/src/lib.rs` | `71a300f9b9bd1acdeadbc6622b9ec6276566107f85685754e95a37643779a979` |
| `crates/fava-publisher-nip01/src/lib.rs` | `3ee36514d45f3bcc12d0f65de16ed3788de5b922837f4a751116cd587eadb501` |
| `crates/fava-publisher/src/lib.rs` | `9af1c5fdea4980aa531de46eb60ef2b4ebd7e65980f161b771caa84b0df51ba9` |
| `crates/fava-write-store/src/receipt.rs` | `9a8dce66a88500cbff82c4727f2b85b033c9f6b23032a6b2550b8f99c2fad904` |
| `crates/fava-write/src/lib.rs` | `1d095acb843f849ae9eb82745838281c0f3297232c36ffad7de478485a49f6e2` |

## Causal RED

The committed public target was run from a temporary `git archive HEAD`, with
the live dirty tree untouched:

```text
CARGO_TARGET_DIR=<temporary>/target cargo test --manifest-path <temporary>/Cargo.toml \
  -p fava --test delivery_bounds

error[E0599]: no method named `spent` found for struct `Receipt`
error[E0560]: struct `Receipt` has no field named `spent_attempts`
error[E0599]: no variant named `Unreachable` found for enum `RelayDeliveryOutcome`
error[E0599]: no variant named `Unreachable` found for enum `PublishOutcome`
```

This is the intended compiled RED: committed `197c278` already demands the
five contract/public facts, and the target cannot compile without their exact
definitions.

## Exit gates

- The five fingerprints above are committed together and match after adoption.
- The public delivery target proves unreachable, real-failure ceiling,
  ambiguous handoff, and acknowledgment without store-provider assumptions.
- `DELIBERATE_BREAK_M8_DELIVERY_IDENTITY_BUDGET` removes the two causal
  identity/budget seams, the public target fails for the named reason, and
  restoration reproduces exact checksums.
- Cargo and Bazel execute the same public target; strict Clippy, vocabulary,
  line, whitespace, and stash-identity gates pass.
