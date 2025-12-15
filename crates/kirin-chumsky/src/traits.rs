use std::fmt::Debug;

use chumsky::prelude::*;
use kirin_ir::Dialect;
use kirin_lexer::Token;

pub trait TokenInput<'tokens, 'src: 'tokens>:
    chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = chumsky::span::SimpleSpan>
{
}

impl<'tokens, 'src: 'tokens, T> TokenInput<'tokens, 'src> for T where
    T: chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = chumsky::span::SimpleSpan>
{
}

pub type ParserError<'tokens, 'src> =
    extra::Err<Rich<'tokens, Token<'src>, chumsky::span::SimpleSpan>>;

pub trait HasParser<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>> {
    type Output: Clone + Debug;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>>;
}
