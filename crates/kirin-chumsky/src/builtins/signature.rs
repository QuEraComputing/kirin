use chumsky::prelude::*;
use kirin_ir::Signature;
use kirin_lexer::Token;

use crate::parsers::return_clause;
use crate::traits::{BoxedParser, DirectlyParsable, HasParser, TokenInput};

impl<T> DirectlyParsable for Signature<T> where T: DirectlyParsable + Clone + PartialEq + 'static {}

impl<'t, T> HasParser<'t> for Signature<T>
where
    T: HasParser<'t, Output = T> + Clone + PartialEq + 'static,
{
    type Output = Signature<T>;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        T::parser()
            .separated_by(just(Token::Comma))
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .then(return_clause(T::parser()))
            .map(|(params, rets)| Signature::new().takes(params).returns_each(rets))
            .labelled("signature")
            .boxed()
    }
}
