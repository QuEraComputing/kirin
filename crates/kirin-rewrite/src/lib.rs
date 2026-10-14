//! Matching and scheduling for Kirin rewrite rules.
//!
//! This crate will own wildcard matching, root-operation indexing, worklists,
//! fixpoints, constraint evaluation, and analysis integration. Concrete IR
//! mutation remains in `kirin-ir`; inspectable `RewriteRule` data remains in
//! `kirin-wildcard`.
//!
//! The public engine API is intentionally not defined until the def-use and
//! replacement-legality contracts in the rewrite-engine design are settled.
#![forbid(unsafe_code)]
