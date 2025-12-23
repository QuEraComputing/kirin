pub mod ast;
mod parsers;
mod traits;

/// Re-export chumsky for parser implementations
pub use chumsky;
pub use kirin_lexer::Token;
pub use parsers::*;
pub use traits::{HasParser, ParserError, TokenInput, WithAbstractSyntaxTree};

pub mod prelude {
    pub use crate::ast;
    pub use crate::parsers::*;
    pub use crate::traits::{HasParser, ParserError, TokenInput, WithAbstractSyntaxTree};
    pub use chumsky::prelude::*;
    pub use kirin_lexer::Token;

    #[cfg(feature = "derive")]
    pub use kirin_derive::WithAbstractSyntaxTree;
}

#[cfg(test)]
mod tests;

#[cfg(feature = "derive")]
pub use kirin_derive::WithAbstractSyntaxTree;
