//! Pure NIP-29 group values over ordinary Fava query and write contracts.

mod bounds;
mod group;
mod query;

#[cfg(test)]
mod tests;

pub use group::{Group, GroupError};
pub use query::GroupRecords;
