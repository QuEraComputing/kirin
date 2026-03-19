use crate::ast::*;
use crate::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

/// Parses an SSA value name (prefixed with `%`).
pub fn ssa_name<'t, I>() -> impl Parser<'t, I, Spanned<&'t str>, ParserError<'t>>
where
    I: TokenInput<'t>,
{
    select! {
        Token::SSAValue(name) = e => Spanned {
            value: name,
            span: e.span(),
        }
    }
    .labelled("SSA value")
}

/// Parses an SSA value with optional type annotation.
///
/// Matches:
/// - `%value`
/// - `%value: type`
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The parser produces `SSAValue<'t, <T as HasParser>::Output>`.
pub fn ssa_value<'t, I, T>()
-> impl Parser<'t, I, SSAValue<'t, <T as HasParser<'t>>::Output>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
{
    ssa_name()
        .then(just(Token::Colon).ignore_then(T::parser()).or_not())
        .map(|(name, ty)| SSAValue { name, ty })
        .labelled("SSA value")
}

/// Parses a result value with optional type annotation.
///
/// Matches:
/// - `%result` (without type)
/// - `%result: type` (with type)
///
/// This is the parser used by format strings with `{result}` (Default option)
/// for ResultValue fields, allowing users to optionally annotate result types.
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The parser produces `ResultValue<'t, <T as HasParser>::Output>`.
pub fn result_value<'t, I, T>()
-> impl Parser<'t, I, ResultValue<'t, <T as HasParser<'t>>::Output>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
{
    ssa_name()
        .then(just(Token::Colon).ignore_then(T::parser()).or_not())
        .map(|(name, ty)| ResultValue {
            name,
            ty,
            result_index: 0,
        })
        .labelled("result value")
}

/// Parses only the name portion of an SSA value.
pub fn nameof_ssa<'t, I>() -> impl Parser<'t, I, NameofSSAValue<'t>, ParserError<'t>>
where
    I: TokenInput<'t>,
{
    select! {
        Token::SSAValue(name) = e => NameofSSAValue {
            name,
            span: e.span(),
        }
    }
    .labelled("SSA value name")
}

/// Parses only the type portion (expects type parser output).
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The parser produces `TypeofSSAValue<<T as HasParser>::Output>`.
pub fn typeof_ssa<'t, I, T>()
-> impl Parser<'t, I, TypeofSSAValue<<T as HasParser<'t>>::Output>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
{
    T::parser()
        .map_with(|ty, extra| TypeofSSAValue {
            ty,
            span: extra.span(),
        })
        .labelled("type annotation")
}

/// Parses a result name list: `%name1, %name2, ... =`
///
/// Used in new-format mode where result names are parsed generically
/// at the statement level rather than by the dialect format string.
///
/// Returns the list of parsed result names. If there are no results
/// (zero-result operation), returns an empty Vec.
pub fn result_name_list<'t, I>()
-> impl Parser<'t, I, Vec<Spanned<&'t str>>, ParserError<'t>>
where
    I: TokenInput<'t>,
{
    ssa_name()
        .separated_by(just(Token::Comma))
        .at_least(1)
        .collect::<Vec<_>>()
        .then_ignore(just(Token::Equal))
        .labelled("result name list")
}

/// Parses a literal integer and converts it using the provided function.
pub fn literal_int<'t, T, I>(
    f: impl Fn(&str, SimpleSpan) -> Result<T, Rich<'t, Token<'t>, SimpleSpan>> + 't,
) -> impl Parser<'t, I, Spanned<T>, ParserError<'t>>
where
    I: TokenInput<'t>,
{
    select! { Token::Int(v) = e => f(v, e.span()) }
        .try_map(|res, span| match res {
            Ok(v) => Ok(Spanned { value: v, span }),
            Err(e) => Err(e),
        })
        .labelled("literal integer")
}

/// Parses a literal float and converts it using the provided function.
pub fn literal_float<'t, T, I>(
    f: impl Fn(&str, SimpleSpan) -> Result<T, Rich<'t, Token<'t>, SimpleSpan>> + 't,
) -> impl Parser<'t, I, Spanned<T>, ParserError<'t>>
where
    I: TokenInput<'t>,
{
    select! { Token::Float(v) = e => f(v, e.span()) }
        .try_map(|res, span| match res {
            Ok(v) => Ok(Spanned { value: v, span }),
            Err(e) => Err(e),
        })
        .labelled("literal float")
}
