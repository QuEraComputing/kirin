use crate::ast::*;
use crate::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

/// Parses a function type signature.
///
/// Matches: `(i32, f64) -> bool`, `(i32) -> bool, i32`, `-> i32`, or `(i32)`
/// for a function with no outputs.
///
/// The outputs use the same grammar as a signature's return clause
/// ([`return_clause`](super::return_clause)): a bare comma list that is never
/// parenthesized, so `-> ()` is one output whose type prints as `()`, and no
/// outputs means no arrow. Because the list is bare, a language that nests a
/// function type inside another comma list must delimit it itself (for
/// example `Fn[(A) -> B, C]`).
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The parser produces `FunctionType<<T as HasParser>::Output>`.
pub fn function_type<'t, I, T>()
-> impl Parser<'t, I, Spanned<FunctionType<<T as HasParser<'t>>::Output>>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
    <T as HasParser<'t>>::Output: Clone,
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

    let output_types = super::return_clause(T::parser().map_with(|ty, e| Spanned {
        value: ty,
        span: e.span(),
    }))
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
