//! Parser combinators for common syntax patterns.

use crate::ast::*;
use crate::traits::*;
use chumsky::prelude::*;
use kirin_ir::Dialect;
use kirin_lexer::Token;

/// Parses a specific identifier keyword.
///
/// # Example
///
/// ```ignore
/// let add_kw = identifier("add"); // matches "add" exactly
/// ```
pub fn identifier<'tokens, 'src: 'tokens, I>(
    name: &'src str,
) -> impl Parser<'tokens, I, Spanned<&'src str>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Identifier(id) = e if id == name => Spanned {
        value: id,
        span: e.span(),
    }}
    .labelled(format!("identifier '{}'", name))
}

/// Parses any identifier.
pub fn any_identifier<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Spanned<&'src str>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Identifier(id) = e => Spanned {
        value: id,
        span: e.span(),
    }}
    .labelled("identifier")
}

/// Parses a symbol (prefixed with `@`).
///
/// # Example
///
/// ```ignore
/// let sym = symbol(); // matches "@foo", returns "foo"
/// ```
pub fn symbol<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Spanned<&'src str>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Symbol(sym) = e => Spanned {
        value: sym,
        span: e.span(),
    }}
    .labelled("symbol")
}

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
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn ssa_value<'tokens, 'src: 'tokens, I, L, T>()
-> impl Parser<'tokens, I, SSAValue<'src, T>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T>,
{
    ssa_name()
        .then(
            just(Token::Colon)
                .ignore_then(T::parser())
                .or_not(),
        )
        .map(|(name, ty)| SSAValue { name, ty })
        .labelled("SSA value")
}

/// Parses an SSA value with required type annotation.
///
/// Matches: `%value: type`
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn ssa_value_with_type<'tokens, 'src: 'tokens, I, L, T>()
-> impl Parser<'tokens, I, SSAValue<'src, T>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T>,
{
    ssa_name()
        .then_ignore(just(Token::Colon))
        .then(T::parser())
        .map(|(name, ty)| SSAValue { name, ty: Some(ty) })
        .labelled("SSA value with type")
}

/// Parses a result value (left-hand side of assignment) without type annotation.
///
/// Matches: `%result`
///
/// For parsing result values with optional type annotations, use
/// [`result_value_with_optional_type`].
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn result_value<'tokens, 'src: 'tokens, I, L, T>()
-> impl Parser<'tokens, I, ResultValue<'src, T>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect,
{
    ssa_name()
        .map(|name| ResultValue { name, ty: None })
        .labelled("result value")
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
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn result_value_with_optional_type<'tokens, 'src: 'tokens, I, L, T>()
-> impl Parser<'tokens, I, ResultValue<'src, T>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T>,
{
    ssa_name()
        .then(
            just(Token::Colon)
                .ignore_then(T::parser())
                .or_not(),
        )
        .map(|(name, ty)| ResultValue { name, ty })
        .labelled("result value")
}

/// Parses multiple result values followed by `=`.
///
/// Matches: `%r1, %r2, %r3 =`
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn result_values<'tokens, 'src: 'tokens, I, L, T>(
    n: usize,
) -> impl Parser<'tokens, I, Vec<ResultValue<'src, T>>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect,
{
    ssa_name()
        .map(|name| ResultValue { name, ty: None })
        .separated_by(just(Token::Comma))
        .exactly(n)
        .collect()
        .then_ignore(just(Token::Equal))
        .labelled(format!("{} result values", n))
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
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn typeof_ssa<'tokens, 'src: 'tokens, I, L, T>()
-> impl Parser<'tokens, I, TypeofSSAValue<T>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T>,
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

/// Parses a block label.
///
/// Matches: `^bb0`
pub fn block_label<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, BlockLabel<'src>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Block(name) = e => Spanned {
        value: name,
        span: e.span(),
    }}
    .map(|name| BlockLabel { name })
    .labelled("block label")
}

/// Type alias for the parsed type AST of a language.
///
/// This is the AST representation of type annotations in the language.
pub type TypeAST<'tokens, 'src, L> = <L as HasDialectParser<'tokens, 'src, L>>::TypeAST;

/// Type alias for the parsed statement output of a language.
pub type StmtOutput<'tokens, 'src, L> = <L as HasDialectParser<'tokens, 'src, L>>::Output;

/// Parses a block argument.
///
/// Matches: `%arg: type`
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn block_argument<'tokens, 'src: 'tokens, I, L, T>()
-> impl Parser<'tokens, I, Spanned<BlockArgument<'src, T>>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T>,
{
    ssa_name()
        .then_ignore(just(Token::Colon))
        .then(T::parser().map_with(|ty, e| Spanned {
            value: ty,
            span: e.span(),
        }))
        .map_with(|(name, ty), e| Spanned {
            value: BlockArgument { name, ty },
            span: e.span(),
        })
        .labelled("block argument")
}

/// Parses a list of block arguments.
///
/// Matches: `(%arg0: i32, %arg1: f64)` or `()` for empty argument lists.
/// Note: Parentheses are always required, even for empty argument lists.
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn block_argument_list<'tokens, 'src: 'tokens, I, L, T>()
-> impl Parser<'tokens, I, Vec<Spanned<BlockArgument<'src, T>>>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T>,
{
    block_argument::<_, L, T>()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .labelled("block arguments")
}

/// Parses a block header.
///
/// Matches: `^bb0(%arg0: i32, %arg1: f64)` or `^bb0()` for blocks with no arguments.
/// Note: Parentheses are always required, even for empty argument lists.
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn block_header<'tokens, 'src: 'tokens, I, L, T>()
-> impl Parser<'tokens, I, Spanned<BlockHeader<'src, T>>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T>,
{
    block_label()
        .then(block_argument_list::<_, L, T>())
        .map_with(|(label, arguments), e| Spanned {
            value: BlockHeader { label, arguments },
            span: e.span(),
        })
        .labelled("block header")
}

/// Parses a complete block with header and statements.
///
/// Requires a parser for the language/dialect statements.
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn block<'tokens, 'src: 'tokens, I, L, T>(
    language: RecursiveParser<
        'tokens,
        'src,
        I,
        StmtOutput<'tokens, 'src, L>,
    >,
) -> impl Parser<'tokens, I, Spanned<Block<'src, T, StmtOutput<'tokens, 'src, L>>>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T>,
{
    let header = block_header::<_, L, T>();
    let statements = language
        .clone()
        .map_with(|stmt, e| Spanned {
            value: stmt,
            span: e.span(),
        })
        .then_ignore(just(Token::Semicolon))
        .repeated()
        .collect::<Vec<_>>()
        .or(empty().to(Vec::new()))
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .labelled("block statements");

    header
        .then(statements)
        .map_with(|(header, statements), e| Spanned {
            value: Block { header, statements },
            span: e.span(),
        })
}

/// Parses a region containing multiple blocks.
///
/// Matches:
/// ```text
/// {
///     ^bb0(%arg: i32) {
///         %x = add %arg, %arg;
///         return %x;
///     }
/// }
/// ```
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn region<'tokens, 'src: 'tokens, I, L, T>(
    language: RecursiveParser<
        'tokens,
        'src,
        I,
        StmtOutput<'tokens, 'src, L>,
    >,
) -> impl Parser<'tokens, I, Region<'src, T, StmtOutput<'tokens, 'src, L>>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T>,
{
    block::<_, L, T>(language)
        .then_ignore(just(Token::Semicolon).or_not())
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .map(|blocks| Region { blocks })
        .labelled("region")
}

/// Parses a region containing multiple blocks, returning a boxed parser.
///
/// This is a convenience function that boxes the region parser for use in recursive contexts.
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn region_boxed<'tokens, 'src: 'tokens, I, L, T>(
    language: RecursiveParser<
        'tokens,
        'src,
        I,
        StmtOutput<'tokens, 'src, L>,
    >,
) -> BoxedParser<'tokens, 'src, I, Region<'src, T, StmtOutput<'tokens, 'src, L>>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T> + 'tokens,
{
    region::<_, L, T>(language).boxed()
}

/// Parses a function type signature.
///
/// Matches: `(i32, f64) -> bool` or `(i32) -> (bool, i32)` or `-> i32`
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn function_type<'tokens, 'src: 'tokens, I, L, T>() -> impl Parser<
    'tokens,
    I,
    Spanned<FunctionType<T>>,
    ParserError<'tokens, 'src>,
>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T> + Clone,
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

/// Parses multiple SSA values separated by a delimiter.
///
/// The type parameter `T` specifies the type annotation type (typically `L::TypeLattice`).
pub fn ssa_values<'tokens, 'src: 'tokens, I, L, T>(
    n: usize,
    sep: Token<'src>,
) -> impl Parser<'tokens, I, Vec<SSAValue<'src, T>>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
    L: HasDialectParser<'tokens, 'src, L> + Dialect + 'tokens,
    T: HasParser<'tokens, 'src, Output = T>,
{
    ssa_value::<_, L, T>()
        .separated_by(just(sep))
        .exactly(n)
        .collect()
        .labelled(format!("{} operands", n))
}
