use chumsky::prelude::*;
use kirin_ir::Signature;
use kirin_lexer::Token;

use crate::parsers::return_type_list;
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
            .then_ignore(just(Token::Arrow))
            .then(return_type_list::<I, T>())
            .map(|(params, rets)| Signature::with_rets(params, rets, ()))
            .labelled("signature")
            .boxed()
    }
}
