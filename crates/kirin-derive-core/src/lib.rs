//! Kirin's derive macro core library
//! 
//! This crate provides the core framework for building derive macros
//! for declaring statements in Kirin IR.
//! 
//! It provides:
//! - trait definitions for defining derive macros by defining [`Layout`](crate::ir::Layout) and
//!   implementing [`Compile`](crate::derive::Compile) and [`Emit`](crate::derive::Emit) traits.
//! - code generation gadgets for common patterns in derive macros.
//! - intermediate representation for parsing and representing statements and their attributes.
//! - miscellaneous utilities for working with syn and quote.
//! 
//! Kirin's built-in derive macros are also implemented using this crate.
//! Take a look at the [`kirin`](crate::kirin) module for more details.

/// traits and tools for derive macro definitions
pub mod derive;
/// code generation gadgets for derive macros
pub mod gadgets;
/// intermediate representation for derive macros and code generation
pub mod ir;
/// miscellaneous utilities
pub mod misc;
/// Kirin's built-in derive macros.
pub mod kirin;

/// Parse derive macros for kirin-chumsky integration
pub mod chumsky;

/// debugging utilities
#[cfg(feature = "debug")]
pub mod debug;

/// commonly used items from kirin-derive-core
pub mod prelude {
    pub use crate::derive::*;
    pub use crate::gadgets::*;
    pub use crate::ir::*;
    pub use crate::misc::*;
    pub use crate::target;
}
