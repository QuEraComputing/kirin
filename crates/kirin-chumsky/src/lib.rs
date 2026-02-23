//! # kirin-chumsky
//!
//! Runtime API for Kirin chumsky parsers, providing traits and common syntax nodes
//! for parsing dialect definitions.
//!
//! # Quick Start
//!
//! Two primary parsing APIs:
//!
//! - **Statement-level** — [`ParseStatementText::parse_statement`] on `StageInfo<L>`:
//!
//!   ```ignore
//!   use kirin_chumsky::prelude::*;
//!
//!   let mut stage: StageInfo<MyLang> = StageInfo::default();
//!   let stmt = stage.parse_statement("%res = add %a, %b")?;
//!   ```
//!
//! - **File-level** — [`ParsePipelineText::parse`] on `Pipeline<StageInfo<L>>`:
//!
//!   ```ignore
//!   use kirin_chumsky::prelude::*;
//!
//!   let mut pipeline: Pipeline<StageInfo<MyLang>> = Pipeline::new();
//!   let functions = pipeline.parse(src)?;
//!   ```
//!
//! For lower-level control, [`parse_ast`] parses text into an AST without emitting
//! IR, and [`EmitContext`] gives full control over SSA name registration.
//!
//! # Defining Dialects
//!
//! ```ignore
//! use kirin::parsers::{HasParser, PrettyPrint};
//! use kirin::ir::Dialect;
//!
//! #[derive(Dialect, HasParser, PrettyPrint)]
//! #[kirin(type = MyType)]
//! pub enum MyDialect {
//!     #[chumsky(format = "{res:name} = add {lhs}, {rhs}")]
//!     Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
//! }
//! ```

pub mod ast;
mod builtins;
mod function_text;
mod parsers;
mod traits;

/// Re-export chumsky for downstream use
pub use chumsky;
pub use kirin_ir as ir;
pub use kirin_lexer::Token;

pub use ast::*;
pub use function_text::*;
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
    pub use crate::ast;
    pub use crate::function_text::{FunctionParseError, FunctionParseErrorKind, ParsePipelineText};
    pub use crate::parsers::*;
    pub use crate::traits::{
        BoxedParser, DirectlyParsable, EmitContext, EmitIR, HasDialectParser, HasParser,
        ParseError, ParseStatementText, ParseStatementTextExt, ParserError, RecursiveParser,
        TokenInput, parse_ast,
    };
    pub use chumsky::prelude::*;
    pub use kirin_lexer::Token;
    pub use kirin_prettyless::prelude::*;

    #[cfg(feature = "derive")]
    pub use kirin_chumsky_derive::{HasParser, PrettyPrint};
}

#[cfg(test)]
mod tests;
