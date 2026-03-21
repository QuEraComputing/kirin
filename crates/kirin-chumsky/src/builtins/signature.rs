use chumsky::prelude::*;
use kirin_ir::Signature;
use kirin_lexer::Token;

use crate::traits::{BoxedParser, HasParser, TokenInput};

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
            .then_ignore(just(Token::Arrow))
            .then(T::parser())
            .map(|(params, ret)| Signature::new(params, ret, ()))
            .labelled("signature")
            .boxed()
    }
}
