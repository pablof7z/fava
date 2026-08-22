//! Pure NIP-29 group values over ordinary Fava query and write contracts.

mod bounds;
mod group;
mod metadata;
mod query;
mod records;

#[cfg(test)]
mod tests;

pub use group::{Group, GroupError};
pub use metadata::GroupMetadata;
pub use query::GroupRecords;
