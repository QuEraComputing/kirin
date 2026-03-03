use chumsky::prelude::*;
use kirin_lexer::Token;

use crate::traits::{BoxedParser, DirectlyParsable, HasParser, TokenInput};

/// Creates a parser for floating point numbers.
pub(super) fn float_parser<'tokens, 'src: 'tokens, T, I>(
    type_name: &'static str,
) -> BoxedParser<'tokens, 'src, I, T>
where
    I: TokenInput<'tokens, 'src>,
    T: std::str::FromStr + Clone + PartialEq + 'tokens,
{
    // Accept both Float tokens and Int tokens (for cases like "1" meaning 1.0)
    let float_token = select! { Token::Float(v) = e => (v, e.span()) };
    let int_token = select! { Token::Int(v) = e => (v, e.span()) };

    float_token
        .or(int_token)
        .try_map(move |(v, span), _| {
            v.parse::<T>()
                .map_err(|_| Rich::custom(span, format!("invalid {} literal: {}", type_name, v)))
        })
        .labelled(type_name)
        .boxed()
}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for f32 {
    type Output = f32;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        float_parser("f32")
    }
}

impl DirectlyParsable for f32 {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for f64 {
    type Output = f64;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        float_parser("f64")
    }
}

impl DirectlyParsable for f64 {}
