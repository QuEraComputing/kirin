use crate::ast::*;
use crate::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

/// Parses an SSA value name (prefixed with `%`).
pub fn ssa_name<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Spanned<&'src str>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
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
/// The parser produces `SSAValue<'src, <T as HasParser>::Output>`.
pub fn ssa_value<'tokens, 'src: 'tokens, I, T>() -> impl Parser<
    'tokens,
    I,
    SSAValue<'src, <T as HasParser<'tokens, 'src>>::Output>,
    ParserError<'tokens, 'src>,
>
where
    I: TokenInput<'tokens, 'src>,
    T: HasParser<'tokens, 'src>,
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
/// The parser produces `ResultValue<'src, <T as HasParser>::Output>`.
pub fn result_value<'tokens, 'src: 'tokens, I, T>() -> impl Parser<
    'tokens,
    I,
    ResultValue<'src, <T as HasParser<'tokens, 'src>>::Output>,
    ParserError<'tokens, 'src>,
>
where
    I: TokenInput<'tokens, 'src>,
    T: HasParser<'tokens, 'src>,
{
    ssa_name()
        .then(just(Token::Colon).ignore_then(T::parser()).or_not())
        .map(|(name, ty)| ResultValue { name, ty })
        .labelled("result value")
}

/// Parses only the name portion of an SSA value.
pub fn nameof_ssa<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, NameofSSAValue<'src>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
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
pub fn typeof_ssa<'tokens, 'src: 'tokens, I, T>() -> impl Parser<
    'tokens,
    I,
    TypeofSSAValue<<T as HasParser<'tokens, 'src>>::Output>,
    ParserError<'tokens, 'src>,
>
where
    I: TokenInput<'tokens, 'src>,
    T: HasParser<'tokens, 'src>,
{
    T::parser()
        .map_with(|ty, extra| TypeofSSAValue {
            ty,
            span: extra.span(),
        })
        .labelled("type annotation")
}

/// Parses a literal integer and converts it using the provided function.
pub fn literal_int<'tokens, 'src: 'tokens, T, I>(
    f: impl Fn(&str, SimpleSpan) -> Result<T, Rich<'tokens, Token<'src>, SimpleSpan>> + 'tokens,
) -> impl Parser<'tokens, I, Spanned<T>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Int(v) = e => f(v, e.span()) }
        .try_map(|res, span| match res {
            Ok(v) => Ok(Spanned { value: v, span }),
            Err(e) => Err(e),
        })
        .labelled("literal integer")
}

/// Parses a literal float and converts it using the provided function.
pub fn literal_float<'tokens, 'src: 'tokens, T, I>(
    f: impl Fn(&str, SimpleSpan) -> Result<T, Rich<'tokens, Token<'src>, SimpleSpan>> + 'tokens,
) -> impl Parser<'tokens, I, Spanned<T>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Float(v) = e => f(v, e.span()) }
        .try_map(|res, span| match res {
            Ok(v) => Ok(Spanned { value: v, span }),
            Err(e) => Err(e),
        })
        .labelled("literal float")
}
