mod block;
mod context;
pub mod digraph;
pub mod error;
mod redefine;
mod region;
mod staged;
pub mod ungraph;

use std::collections::HashMap;

use crate::node::ssa::BuilderKey;
use crate::node::symbol::Symbol;
use crate::InternTable;

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
