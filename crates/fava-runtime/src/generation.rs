//! Operation generation identity, owned by `fava-query`.
//!
//! `FROZEN-CONTRACTS.md` §0 places `OperationGeneration` in `fava-query` so
//! every crate that carries one carries the same noun. `fava-runtime`
//! interprets the value only to hand it from the authorising call back to the
//! owner's completion.

pub use fava_query::OperationGeneration;
