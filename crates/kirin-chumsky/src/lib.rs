pub mod ast;
mod parser;
mod traits;

pub use kirin_lexer::Token;
pub use parser::*;
pub use traits::{HasParser, ParserError, TokenInput};

#[cfg(test)]
mod tests;
