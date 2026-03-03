//! Code generation utilities for derive macros.
//!
//! This module provides common helpers for generating code patterns
//! that are frequently needed in derive macro implementations.

mod constructor;
mod field_bindings;
mod generics_builder;
mod utils;

pub use constructor::ConstructorBuilder;
pub use field_bindings::FieldBindings;
pub use generics_builder::GenericsBuilder;
pub use utils::{combine_where_clauses, deduplicate_types};
