//! IR construction via [`BuilderStageInfo`].
//!
//! The builder module provides the mutable construction API for IR nodes.
//! All construction goes through [`BuilderStageInfo`], which holds build-time
//! arenas with placeholder support ([`BuilderSSAInfo`](crate::BuilderSSAInfo),
//! [`BuilderSSAKind`](crate::BuilderSSAKind)).
//!
//! # Workflow
//!
//! ```text
//! BuilderStageInfo::default()
//!   → stage.statement()          create statements
//!   → stage.block_argument()     create placeholder SSAs
//!   → stage.block()              build blocks (resolves placeholders)
//!   → stage.region()             group blocks into regions
//!   → stage.staged_function()    register callable functions
//!   → stage.specialize()         add specializations
//!   → stage.finalize()           validate → StageInfo (clean SSAInfo)
//! ```
//!
//! See [`BuilderStageInfo`] for detailed usage examples.

mod block;
mod context;
pub mod digraph;
pub mod error;
mod redefine;
mod region;
mod stage_info;
mod staged;
pub mod ungraph;

pub use stage_info::{BuilderStageInfo, FinalizeError};

use std::collections::HashMap;

use crate::InternTable;
use crate::node::ssa::BuilderKey;
use crate::node::symbol::Symbol;

/// Resolve a `BuilderKey` to a positional index.
///
/// Panics if the key is out of bounds or the name is not found.
fn resolve_builder_key(
    key: BuilderKey,
    len: usize,
    name_to_index: &HashMap<Symbol, usize>,
    symbols: &InternTable<String, Symbol>,
    context: &str,
) -> usize {
    match key {
        BuilderKey::Index(i) => {
            assert!(
                i < len,
                "{context} index {i} out of bounds (declared {len})"
            );
            i
        }
        BuilderKey::Named(sym) => {
            if let Some(&i) = name_to_index.get(&sym) {
                i
            } else {
                let name = symbols
                    .resolve(sym)
                    .cloned()
                    .unwrap_or_else(|| format!("<symbol:{}>", usize::from(sym)));
                panic!("{context} named \"{name}\" not found in builder declarations");
            }
        }
    }
}
