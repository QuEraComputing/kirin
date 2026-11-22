//! Parser combinators for common syntax patterns.

mod blocks;
mod function_type;
mod graphs;
mod identifiers;
mod return_types;
mod values;

pub use blocks::*;
pub use function_type::*;
pub use graphs::*;
pub use identifiers::*;
pub use return_types::*;
pub use values::*;
