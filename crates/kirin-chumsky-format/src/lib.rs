//! # kirin-chumsky-format
//!
//! This crate provides the format parsing and code generation for the Kirin
//! chumsky derive macros.
//!
//! It parses format strings like `"{res} = add {lhs} {rhs}"` and generates
//! AST types and parser implementations.

mod attrs;
mod format;
mod generate;

pub use attrs::{ChumskyFieldAttrs, ChumskyGlobalAttrs, ChumskyStatementAttrs};
pub use format::{Format, FormatElement, FormatOption};
pub use generate::{GenerateHasRecursiveParser, GenerateWithAbstractSyntaxTree};

use kirin_derive_core_2::ir::Layout;

/// The layout for chumsky derive macros.
#[derive(Debug, Clone)]
pub struct ChumskyLayout;

impl Layout for ChumskyLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = ChumskyGlobalAttrs;
    type ExtraStatementAttrs = ChumskyStatementAttrs;
    type ExtraFieldAttrs = ChumskyFieldAttrs;
}
