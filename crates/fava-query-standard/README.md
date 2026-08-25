# fava-query-standard

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_query_standard` (Module)

Compiler-visible module `fava_query_standard`.
<!-- api-item {"kind":"Module","item":"fava_query_standard","signature":"pub mod fava_query_standard","evidence":"cargo-public-api@0.52.0: pub mod fava_query_standard"} -->

### `StandardQueryEvaluator` (Struct)

Compiler-visible struct `fava_query_standard::StandardQueryEvaluator`.
<!-- api-item {"kind":"Struct","item":"fava_query_standard::StandardQueryEvaluator","signature":"pub struct fava_query_standard::StandardQueryEvaluator","evidence":"cargo-public-api@0.52.0: pub struct fava_query_standard::StandardQueryEvaluator"} -->

| Item | Purpose |
| --- | --- |
| **`fava_query::QueryEvaluator::evaluate`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_query_standard::StandardQueryEvaluator as fava_query::QueryEvaluator>::evaluate","signature":"pub fn fava_query_standard::StandardQueryEvaluator::evaluate(&self, &fava_query::Query, &[fava_query::SourceSnapshot]) -> core::result::Result<fava_query::QuerySnapshot, fava_query::QueryEvaluationError>","evidence":"cargo-public-api@0.52.0: pub fn fava_query_standard::StandardQueryEvaluator::evaluate(&self, &fava_query::Query, &[fava_query::SourceSnapshot]) -> core::result::Result<fava_query::QuerySnapshot, fava_query::QueryEvaluationError>"} --> | Compiler-visible method owned by `fava_query_standard::StandardQueryEvaluator`. |
<!-- END crate-readme-api inventory -->
