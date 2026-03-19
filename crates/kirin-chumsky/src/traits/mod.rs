//! Core traits for Kirin chumsky parsers

mod emit_ir;
mod has_dialect_emit_ir;
mod has_parser;
mod has_parser_emit_ir;
mod parse_emit;
mod parse_text;

use chumsky::prelude::*;
use chumsky::recursive::{Direct, Recursive};
use kirin_lexer::Token;

/// Standard error type for Kirin chumsky parsers.
pub type ParserError<'t> = extra::Err<Rich<'t, Token<'t>, SimpleSpan>>;

/// Type alias for a boxed parser.
pub type BoxedParser<'t, I, O> = Boxed<'t, 't, I, O, ParserError<'t>>;

/// Type alias for a recursive parser handle.
pub type RecursiveParser<'t, I, O> = Recursive<Direct<'t, 't, I, O, ParserError<'t>>>;

pub use emit_ir::*;
pub use has_dialect_emit_ir::*;
pub use has_parser::*;
pub use has_parser_emit_ir::*;
pub use parse_emit::{ChumskyError, ParseEmit, SimpleParseEmit};
pub use parse_text::*;
