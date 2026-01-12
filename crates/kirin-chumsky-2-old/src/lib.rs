pub mod parsers;

pub use chumsky;
pub use kirin_ir as ir;
pub use kirin_lexer::Token;

pub use parsers::*;

#[cfg(test)]
mod tests;

pub mod prelude {
    pub use crate::parsers::*;
    pub use crate::{
        BoxedParser, LanguageChumskyParser, ParserError, RecursiveParser, TokenInput,
        WithChumskyParser, WithRecursiveChumskyParser,
    };
    pub use chumsky::prelude::*;
    pub use kirin_lexer::Token;
}
