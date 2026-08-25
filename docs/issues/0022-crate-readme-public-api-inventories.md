# 0022 — Crate READMEs can silently drift from their exported Rust APIs

**Status:** implemented, awaiting review of the `fava-simple-groups` seed
**Raised:** 2026-08-24, by Pablo

## Problem

Crate READMEs describe public values by hand. Rust exports can add or remove
modules, values, methods, enum variants, fields, macros, or re-exported paths
without requiring that prose to change, so a nominal-type-only check cannot
prove that the README inventory is current.

## Resolution

`tools/crate_readme_api.py` derives each workspace library crate's exported API
with an exact `cargo-public-api` version and an exact nightly rustdoc toolchain.
It uses all features, includes documentation-hidden public exports, and omits
only compiler-generated blanket, auto-trait, and auto-derived implementations.
The resulting managed table includes modules, nominal and alias types, traits,
functions, inherent and explicitly implemented trait methods, constants,
statics, enum variants, named and positional public fields, macros, and every
re-export at its exported path.

Only the table's kind and item columns are generated. Per-item descriptions are
preserved by stable `(kind, exported path)` identity, and bytes outside the
marker pair are not rewritten. CI maps the pull request or push diff to
workspace crate directories and rejects each modified library crate whose
README inventory is absent or stale. A root workspace manifest change checks
all library crates.

`fava-simple-groups` is the first generated inventory. Other crates acquire a
table when they are next modified; CI does not claim that unmodified crate
READMEs have already been migrated.
