//! Live-query ownership: installed observation identity, retained logical relay
//! demand per observation, desired wire subscription plans, scoped relay
//! evidence, and bounded latest-state delivery to applications.
//!
//! Opening an observation is synchronous and total. It establishes the local
//! source boundary, binds the route plan, compiles logical demand, evaluates one
//! complete initial snapshot, installs the observation, and releases the handle.
//! Relay work is reconciled afterwards by an owner-held engine; no provider call
//! is awaited before the handle exists.
//!
//! Two equivalent observations are never collapsed here. Each keeps its own
//! `DemandId`, bounds, route origin, and evidence; the planner receives all of
//! them and decides whether one REQ can carry them (`GOALS:296`, QUERY-002).
//! The connection is shared by the transport's lease registry, and a wire
//! subscription survives until the last logical demand it serves withdraws.

mod admission;
mod completions;
mod diagnostics;
mod engine;
mod error;
mod facts;
mod ingest;
mod observation;
mod observer;
mod operations;
mod registry;
mod routes;
mod slot;
mod sources;

pub use error::{ObservationClosed, ObserveError};
/// Cross-crate read-side identity, re-exported from its neutral home. This
/// crate is the only one that mints values.
pub use fava_query::{ObservationId, QueryBranchId};
pub use observation::Observation;
pub use observer::Observer;
