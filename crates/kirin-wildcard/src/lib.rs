//! Inspectable wildcard IR and rewrite-rule data for Kirin.
//!
//! This crate will own wildcard bindings, match and replacement fragments,
//! `RewriteRule` data, and their textual representation. It does not mutate
//! concrete IR or schedule rewrites; those responsibilities belong to
//! `kirin-ir` and `kirin-rewrite`, respectively.
//!
//! The public data model is intentionally not defined until the def-use and
//! replacement-legality contracts in the rewrite-engine design are settled.
#![forbid(unsafe_code)]
