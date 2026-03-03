//! Document builder for pretty printing.
//!
//! The [`Document`] type is the main entry point for manual pretty printing.
//! It wraps a `prettyless::Arena` allocator and provides methods for building
//! document trees from IR nodes.

pub(crate) mod builder;
mod ir_render;

pub use builder::Document;

#[cfg(test)]
mod tests;
