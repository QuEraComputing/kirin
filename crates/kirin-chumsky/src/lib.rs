//! # kirin-chumsky
//!
//! Runtime API for Kirin chumsky parsers, providing traits and common syntax nodes
//! for parsing dialect definitions.
//!
//! This crate provides:
//! - Core traits: `HasParser`, `HasDialectParser`, `EmitIR`
//! - Common syntax nodes: `Spanned`, `SSAValue`, `ResultValue`, `Block`, `Region`, etc.
//! - Parser combinators for common syntaxes
//! - IR emission via `EmitContext` and `EmitIR` trait
//!
//! # Usage
//!
//! Import both `HasParser` and `PrettyPrint` traits/derives for full dialect support:
//!
//! ```ignore
//! use kirin::parsers::{HasParser, PrettyPrint};
//! use kirin::ir::Dialect;
//!
//! #[derive(Dialect, HasParser, PrettyPrint)]
//! #[kirin(type_lattice = MyType)]
//! pub enum MyDialect {
//!     #[chumsky(format = "{res:name} = add {lhs}, {rhs}")]
//!     Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
//! }
//! ```
//!
//! - `HasParser` enables parsing via `MyDialect::parser()` or `parse::<MyDialect>(...)`
//! - `PrettyPrint` enables roundtrip-compatible printing

mod ast;
mod parsers;
mod traits;

/// Re-export chumsky for downstream use
pub use chumsky;
pub use kirin_ir as ir;
pub use kirin_lexer::Token;

pub use ast::*;
pub use parsers::*;
pub use traits::*;

// Re-export PrettyPrint trait from kirin_prettyless
pub use kirin_prettyless::PrettyPrint;

// When derive feature is enabled, also export derive macros with the same names as traits
// This allows `use kirin::parsers::HasParser` to import both trait AND derive
#[cfg(feature = "derive")]
pub use kirin_chumsky_derive::HasParser;

#[cfg(feature = "derive")]
pub use kirin_chumsky_derive::PrettyPrint;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::ast::*;
    pub use crate::parsers::*;
    pub use crate::traits::*;
    pub use crate::{emit, parse, parse_ast, EmitContext, EmitIR, ParseError};
    pub use chumsky::prelude::*;
    pub use kirin_lexer::Token;
    pub use kirin_prettyless::PrettyPrint;

    #[cfg(feature = "derive")]
    pub use kirin_chumsky_derive::{HasParser, PrettyPrint as DerivePrettyPrint};
}

#[cfg(test)]
mod tests;
