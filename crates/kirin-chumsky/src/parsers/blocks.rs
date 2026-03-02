use crate::ast::*;
use crate::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

use super::values::ssa_name;

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

/// Type alias for the parsed statement output of a dialect.
///
/// The `D` parameter is the dialect being parsed.
/// The `TypeOutput` parameter is the parsed type representation.
/// The `LanguageOutput` parameter is the AST type for nested statements.
pub type StmtOutput<'tokens, 'src, D, TypeOutput, LanguageOutput> =
    <D as HasDialectParser<'tokens, 'src>>::Output<TypeOutput, LanguageOutput>;

/// Parses a block argument.
///
/// Matches: `%arg: type`
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The parser produces `BlockArgument<'src, <T as HasParser>::Output>`.
pub fn block_argument<'tokens, 'src: 'tokens, I, T>() -> impl Parser<
    'tokens,
    I,
    Spanned<BlockArgument<'src, <T as HasParser<'tokens, 'src>>::Output>>,
    ParserError<'tokens, 'src>,
>
where
    I: TokenInput<'tokens, 'src>,
    T: HasParser<'tokens, 'src>,
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
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The parser produces `Vec<Spanned<BlockArgument<'src, <T as HasParser>::Output>>>`.
pub fn block_argument_list<'tokens, 'src: 'tokens, I, T>() -> impl Parser<
    'tokens,
    I,
    Vec<Spanned<BlockArgument<'src, <T as HasParser<'tokens, 'src>>::Output>>>,
    ParserError<'tokens, 'src>,
>
where
    I: TokenInput<'tokens, 'src>,
    T: HasParser<'tokens, 'src>,
{
    block_argument::<_, T>()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .labelled("block arguments")
}

/// Parses a block header.
///
/// Matches:
/// - `^bb0(%arg0: i32, %arg1: f64)`
/// - `^bb0()` for explicit empty argument lists
/// - `^bb0` for omitted empty argument lists
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The parser produces `BlockHeader<'src, <T as HasParser>::Output>`.
pub fn block_header<'tokens, 'src: 'tokens, I, T>() -> impl Parser<
    'tokens,
    I,
    Spanned<BlockHeader<'src, <T as HasParser<'tokens, 'src>>::Output>>,
    ParserError<'tokens, 'src>,
>
where
    I: TokenInput<'tokens, 'src>,
    T: HasParser<'tokens, 'src>,
{
    let arguments = block_argument_list::<_, T>()
        .or_not()
        .map(|args| args.unwrap_or_default());

    block_label()
        .then(arguments)
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
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The type parameter `S` is the statement AST type produced by the language parser.
/// The parser produces `Block<'src, <T as HasParser>::Output, S>`.
pub fn block<'tokens, 'src: 'tokens, I, T, S>(
    language: RecursiveParser<'tokens, 'src, I, S>,
) -> impl Parser<
    'tokens,
    I,
    Spanned<Block<'src, <T as HasParser<'tokens, 'src>>::Output, S>>,
    ParserError<'tokens, 'src>,
>
where
    I: TokenInput<'tokens, 'src>,
    T: HasParser<'tokens, 'src>,
    S: Clone,
{
    let header = block_header::<_, T>();
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
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The type parameter `S` is the statement AST type produced by the language parser.
/// The parser produces `Region<'src, <T as HasParser>::Output, S>`.
pub fn region<'tokens, 'src: 'tokens, I, T, S>(
    language: RecursiveParser<'tokens, 'src, I, S>,
) -> impl Parser<
    'tokens,
    I,
    Region<'src, <T as HasParser<'tokens, 'src>>::Output, S>,
    ParserError<'tokens, 'src>,
>
where
    I: TokenInput<'tokens, 'src>,
    T: HasParser<'tokens, 'src>,
    S: Clone,
{
    block::<_, T, S>(language)
        .then_ignore(just(Token::Semicolon).or_not())
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .map(|blocks| Region { blocks })
        .labelled("region")
}
