use crate::ast::*;
use crate::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

/// Parses a function type signature.
///
/// Matches: `(i32, f64) -> bool` or `(i32) -> (bool, i32)` or `-> i32`
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The parser produces `FunctionType<<T as HasParser>::Output>`.
pub fn function_type<'tokens, 'src: 'tokens, I, T>() -> impl Parser<
    'tokens,
    I,
    Spanned<FunctionType<<T as HasParser<'tokens, 'src>>::Output>>,
    ParserError<'tokens, 'src>,
>
where
    I: TokenInput<'tokens, 'src>,
    T: HasParser<'tokens, 'src>,
    <T as HasParser<'tokens, 'src>>::Output: Clone,
{
    let input_types = T::parser()
        .map_with(|ty, e| Spanned {
            value: ty,
            span: e.span(),
        })
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .or(empty().to(Vec::new()))
        .labelled("function input types");

    let output_types = just(Token::Arrow)
        .ignore_then(
            T::parser()
                .map_with(|ty, e| Spanned {
                    value: ty,
                    span: e.span(),
                })
                .separated_by(just(Token::Comma))
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .or(T::parser().map_with(|ty, e| {
                    vec![Spanned {
                        value: ty,
                        span: e.span(),
                    }]
                }))
                .or(empty().to(Vec::new())),
        )
        .or(empty().to(Vec::new()))
        .labelled("function output types");

    input_types
        .then(output_types)
        .map_with(|(input_types, output_types), e| Spanned {
            value: FunctionType {
                input_types,
                output_types,
            },
            span: e.span(),
        })
}
