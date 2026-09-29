//! Test-only scaffolding, compiled out of every non-test build.
//!
//! A small [`dialect`] for in-crate tests to build IR with. Helpers that forge
//! broken derived metadata live beside the tests that need them, not here.

pub(crate) mod dialect;
