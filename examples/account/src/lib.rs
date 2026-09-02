//! Library target for `account`, present only so `cargo test --doc` has a
//! target to run against.
//!
//! `account` is a binary example: `src/main.rs` owns every module. Cargo's
//! doc tests only run against a library target, and none exists for a
//! binary-only package, so `cargo test --doc` fails with "no library
//! targets found" rather than reporting zero doc tests. This file carries
//! no code of its own.
