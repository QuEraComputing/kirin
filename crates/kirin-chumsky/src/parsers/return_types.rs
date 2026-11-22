use crate::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

/// Parses a bare, possibly empty, comma-separated list of return types:
/// `T`, `T, U`, or nothing.
///
/// The list is never parenthesized, so a type that itself prints with
/// parentheses (a unit `()` or a tuple `(T, U)`) is always one return.
pub fn return_type_list<'t, I, O>(
    ty: impl Parser<'t, I, O, ParserError<'t>> + Clone,
) -> impl Parser<'t, I, Vec<O>, ParserError<'t>> + Clone
where
    I: TokenInput<'t>,
{
    ty.separated_by(just(Token::Comma))
        .collect::<Vec<_>>()
        .labelled("return types")
}

/// Parses the optional return clause of a signature: `-> T`, `-> T, U`, or
/// nothing at all for a function with no returns.
pub fn return_clause<'t, I, O>(
    ty: impl Parser<'t, I, O, ParserError<'t>> + Clone,
) -> impl Parser<'t, I, Vec<O>, ParserError<'t>> + Clone
where
    I: TokenInput<'t>,
{
    just(Token::Arrow)
        .ignore_then(
            ty.separated_by(just(Token::Comma))
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .or_not()
        .map(Option::unwrap_or_default)
        .labelled("return types")
}
