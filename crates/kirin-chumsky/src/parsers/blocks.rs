use crate::ast::*;
use crate::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

use super::values::ssa_name;

/// Parses a block label.
///
/// Matches: `^bb0`
pub fn block_label<'t, I>() -> impl Parser<'t, I, BlockLabel<'t>, ParserError<'t>>
where
    I: TokenInput<'t>,
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
pub type StmtOutput<'t, D, TypeOutput, LanguageOutput> =
    <D as HasDialectParser<'t>>::Output<TypeOutput, LanguageOutput>;

/// Parses a block argument.
///
/// Matches: `%arg: type`
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The parser produces `BlockArgument<'t, <T as HasParser>::Output>`.
pub fn block_argument<'t, I, T>()
-> impl Parser<'t, I, Spanned<BlockArgument<'t, <T as HasParser<'t>>::Output>>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
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
/// The parser produces `Vec<Spanned<BlockArgument<'t, <T as HasParser>::Output>>>`.
pub fn block_argument_list<'t, I, T>()
-> impl Parser<'t, I, Vec<Spanned<BlockArgument<'t, <T as HasParser<'t>>::Output>>>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
{
    block_argument::<_, T>()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .labelled("block arguments")
}

/// Parses a bare list of block arguments (no surrounding parentheses).
///
/// Matches: `%arg0: i32, %arg1: f64` or empty
///
/// Use this for `:args` body projections where the caller provides delimiter tokens
/// via the format string.
pub fn block_argument_list_bare<'t, I, T>()
-> impl Parser<'t, I, Vec<Spanned<BlockArgument<'t, <T as HasParser<'t>>::Output>>>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
{
    block_argument::<_, T>()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .labelled("block arguments (bare)")
}

/// Parses a block header.
///
/// Matches:
/// - `^bb0(%arg0: i32, %arg1: f64)`
/// - `^bb0()` for explicit empty argument lists
/// - `^bb0` for omitted empty argument lists
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The parser produces `BlockHeader<'t, <T as HasParser>::Output>`.
pub fn block_header<'t, I, T>()
-> impl Parser<'t, I, Spanned<BlockHeader<'t, <T as HasParser<'t>>::Output>>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
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

/// Parses a block with header and statements, **without** the `block`
/// discriminator keyword.
///
/// Matches: `^bb0(%arg: i32) { ... }`
///
/// This is the shared block grammar. It is internal on purpose: the two public
/// entry points differ only in what wraps it.
///
/// - [`block`] prefixes the `block` keyword — a standalone `Block` body is
///   always tagged.
/// - [`cfg()`] and [`cfg_body()`] use it directly — a CFG's member blocks stay
///   untagged, because the enclosing `cfg { ... }` already names the body kind.
///
/// Requires a parser for the language/dialect statements.
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The type parameter `S` is the statement AST type produced by the language parser.
/// The parser produces `Block<'t, <T as HasParser>::Output, S>`.
fn untagged_block<'t, I, T, S>(
    language: RecursiveParser<'t, I, S>,
) -> impl Parser<'t, I, Spanned<Block<'t, <T as HasParser<'t>>::Output, S>>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
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

    header.then(statements).map_with(|(header, statements), e| {
        let h = header.value;
        Spanned {
            value: Block {
                label: Some(h.label.name),
                arguments: h.arguments,
                statements,
            },
            span: e.span(),
        }
    })
}

/// Parses a complete standalone `Block` body, including its `block` discriminator.
///
/// Matches:
/// ```text
/// block ^bb0(%arg: i32) {
///     %x = add %arg, %arg;
///     ret %x;
/// }
/// ```
///
/// This is the parser behind the default `{body}` interpolation of a `Block`
/// field. The `block` keyword is **required** — the untagged form
/// `^bb0(...) { ... }` is only valid for a CFG's member blocks, where
/// [`cfg()`] supplies the discriminator once for the whole body.
///
/// Requires a parser for the language/dialect statements.
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The type parameter `S` is the statement AST type produced by the language parser.
/// The parser produces `Block<'t, <T as HasParser>::Output, S>`.
pub fn block<'t, I, T, S>(
    language: RecursiveParser<'t, I, S>,
) -> impl Parser<'t, I, Spanned<Block<'t, <T as HasParser<'t>>::Output, S>>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
    S: Clone,
{
    just(Token::Identifier("block"))
        .ignore_then(untagged_block::<_, T, S>(language))
        .labelled("block")
}

/// Parses a CFG containing multiple blocks, including its `cfg` discriminator.
///
/// Matches:
/// ```text
/// cfg {
///     ^bb0(%arg: i32) {
///         %x = add %arg, %arg;
///         ret %x;
///     }
/// }
/// ```
///
/// This is the parser behind the default `{body}` interpolation of a `CFG`
/// field. The `cfg` keyword is **required**; the member blocks are untagged
/// (`^bb0 { ... }`, never `block ^bb0 { ... }`) because `cfg` already names
/// the body kind for the whole container.
///
/// The type parameter `T` specifies the type annotation type (typically the TypeLattice).
/// The type parameter `S` is the statement AST type produced by the language parser.
/// The parser produces `CFG<'t, <T as HasParser>::Output, S>`.
pub fn cfg<'t, I, T, S>(
    language: RecursiveParser<'t, I, S>,
) -> impl Parser<'t, I, CFG<'t, <T as HasParser<'t>>::Output, S>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
    S: Clone,
{
    just(Token::Identifier("cfg"))
        .ignore_then(
            untagged_block::<_, T, S>(language)
                .then_ignore(just(Token::Semicolon).or_not())
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|blocks| CFG { blocks })
        .labelled("cfg")
}

/// Parses block body statements (without header, without braces).
///
/// Matches a sequence of `statement ;` pairs. This is the inner content of
/// a block body, used for `{field:body}` projections on Block fields where the
/// caller provides surrounding syntax via the format string. It stays raw: no
/// `block` keyword and no braces are injected.
pub fn block_body_statements<'t, I, S>(
    language: RecursiveParser<'t, I, S>,
) -> impl Parser<'t, I, Vec<Spanned<S>>, ParserError<'t>>
where
    I: TokenInput<'t>,
    S: Clone,
{
    language
        .map_with(|stmt, e| Spanned {
            value: stmt,
            span: e.span(),
        })
        .then_ignore(just(Token::Semicolon))
        .repeated()
        .collect::<Vec<_>>()
        .labelled("block body statements")
}

/// Parses CFG body (untagged blocks, without the `cfg` keyword or outer braces).
///
/// Matches a sequence of untagged blocks, each optionally terminated by a
/// semicolon. This is the inner content of a CFG, used for `{field:body}`
/// projections on CFG fields where the caller provides surrounding syntax via
/// the format string. It stays raw: no `cfg` keyword is injected, so a dialect
/// author can spell their own wrapper.
pub fn cfg_body<'t, I, T, S>(
    language: RecursiveParser<'t, I, S>,
) -> impl Parser<'t, I, Vec<Spanned<Block<'t, <T as HasParser<'t>>::Output, S>>>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
    S: Clone,
{
    untagged_block::<_, T, S>(language)
        .then_ignore(just(Token::Semicolon).or_not())
        .repeated()
        .collect::<Vec<_>>()
        .labelled("cfg body")
}
