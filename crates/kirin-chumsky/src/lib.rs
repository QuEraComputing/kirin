//! # kirin-chumsky
//!
//! Runtime API for Kirin chumsky parsers, providing traits and common syntax nodes
//! for parsing dialect definitions.
//!
//! This crate provides:
//! - Core traits: `HasParser`, `HasRecursiveParser`, `WithAbstractSyntaxTree`, `EmitIR`
//! - Common syntax nodes: `Spanned`, `SSAValue`, `ResultValue`, `Block`, `Region`, etc.
//! - Parser combinators for common syntaxes
//! - IR emission via `EmitContext` and `EmitIR` trait

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

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::ast::*;
    pub use crate::parsers::*;
    pub use crate::traits::*;
    pub use crate::{emit, parse, parse_ast, EmitContext, EmitIR, ParseError};
    pub use chumsky::prelude::*;
    pub use kirin_lexer::Token;

    #[cfg(feature = "derive")]
    pub use kirin_chumsky_derive::{EmitIR as DeriveEmitIR, HasRecursiveParser, WithAbstractSyntaxTree, DialectParser};
}

#[cfg(feature = "derive")]
pub use kirin_chumsky_derive::{EmitIR as DeriveEmitIR, HasRecursiveParser, WithAbstractSyntaxTree, DialectParser};

#[cfg(test)]
mod tests;
