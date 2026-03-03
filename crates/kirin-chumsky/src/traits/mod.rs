//! Core traits for Kirin chumsky parsers

mod emit_ir;
mod has_parser;
mod parse_text;

use chumsky::prelude::*;
use chumsky::recursive::{Direct, Recursive};
use kirin_lexer::Token;

/// Standard error type for Kirin chumsky parsers.
pub type ParserError<'tokens, 'src> = extra::Err<Rich<'tokens, Token<'src>, SimpleSpan>>;

/// Type alias for a boxed parser.
pub type BoxedParser<'tokens, 'src, I, O> =
    Boxed<'tokens, 'tokens, I, O, ParserError<'tokens, 'src>>;

/// Type alias for a recursive parser handle.
pub type RecursiveParser<'tokens, 'src, I, O> =
    Recursive<Direct<'tokens, 'tokens, I, O, ParserError<'tokens, 'src>>>;

pub use emit_ir::*;
pub use has_parser::*;
pub use parse_text::*;
