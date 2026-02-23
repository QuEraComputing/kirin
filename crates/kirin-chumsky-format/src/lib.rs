//! # kirin-chumsky-format
//!
//! This crate provides the format parsing and code generation for the Kirin
//! chumsky derive macros.
//!
//! It parses format strings like `"{res} = add {lhs} {rhs}"` and generates
//! AST types and parser implementations.

mod attrs;
mod field_kind;
mod format;
mod generate;
mod generics;
mod input;
mod validation;
mod visitor;

pub use attrs::{ChumskyFieldAttrs, ChumskyGlobalAttrs, ChumskyStatementAttrs, PrettyGlobalAttrs};
pub use field_kind::{FieldKind, collect_fields};
pub use format::{Format, FormatElement, FormatOption};
pub use generate::{GenerateAST, GenerateEmitIR, GenerateHasDialectParser, GeneratePrettyPrint};
pub use generics::GenericsBuilder;
pub use input::{parse_derive_input, parse_pretty_derive_input};
pub use validation::{FieldOccurrence, ValidationResult, ValidationVisitor, validate_format};
pub use visitor::{FormatVisitor, VisitorContext, visit_format};

use kirin_derive_core::ir::Layout;

/// The layout for chumsky derive macros.
#[derive(Debug, Clone)]
pub struct ChumskyLayout;

impl Layout for ChumskyLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = ChumskyGlobalAttrs;
    type ExtraStatementAttrs = ChumskyStatementAttrs;
    type ExtraFieldAttrs = ChumskyFieldAttrs;
}

/// The layout for the `PrettyPrint` derive macro.
///
/// Reuses `ChumskyStatementAttrs` and `ChumskyFieldAttrs` for format strings,
/// but uses `PrettyGlobalAttrs` for the `#[pretty(crate = ...)]` attribute.
#[derive(Debug, Clone)]
pub struct PrettyPrintLayout;

impl Layout for PrettyPrintLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = PrettyGlobalAttrs;
    type ExtraStatementAttrs = ChumskyStatementAttrs;
    type ExtraFieldAttrs = ChumskyFieldAttrs;
}
