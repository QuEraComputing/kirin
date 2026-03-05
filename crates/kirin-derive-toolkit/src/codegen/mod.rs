//! Utilities for generating constructor expressions, managing generics, and
//! binding field variables in generated code.

mod constructor;
mod field_bindings;
mod generics_builder;
mod utils;

pub use constructor::ConstructorBuilder;
pub use field_bindings::FieldBindings;
pub use generics_builder::GenericsBuilder;
pub use utils::{combine_where_clauses, deduplicate_types};
