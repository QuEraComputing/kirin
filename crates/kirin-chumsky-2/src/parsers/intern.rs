use super::traits::ChumskyParser;
use chumsky::prelude::*;
use kirin_lexer::Token;

/// a symbol in the source code, or a piece of interned string
/// with syntax `@name`.
#[derive(Debug, Clone, PartialEq)]
pub struct Symbol<'src> {
    pub name: &'src str,
    pub span: SimpleSpan,
}

pub fn symbol<'tokens, 'src: 'tokens, I>() -> impl ChumskyParser<'tokens, 'src, I, Symbol<'src>>
where
    'src: 'tokens,
    I: super::traits::TokenInput<'tokens, 'src>,
{
    select! { Token::Symbol(name) = e => Symbol {
        name,
        span: e.span(),
    } }
    .labelled("symbol")
}
