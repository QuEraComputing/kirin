use crate::ast::*;
use crate::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

use super::values::ssa_name;

/// Parses a `capture(...)` clause.
///
/// Matches: `capture(%name: Type, ...)`
fn capture_clause<'t, I, T>(
) -> impl Parser<'t, I, Vec<Spanned<BlockArgument<'t, <T as HasParser<'t>>::Output>>>, ParserError<'t>>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
{
    just(Token::Identifier("capture"))
        .ignore_then(super::block_argument_list::<_, T>())
        .labelled("capture clause")
}

/// Parses a graph header: `^name(%port: Type, ...) [capture(%cap: Type, ...)]`
fn graph_header<'t, I, T>(
) -> impl Parser<
    't,
    I,
    Spanned<GraphHeader<'t, <T as HasParser<'t>>::Output>>,
    ParserError<'t>,
>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
{
    let ports = super::block_argument_list::<_, T>()
        .or_not()
        .map(|args| args.unwrap_or_default());

    let captures = capture_clause::<_, T>()
        .or_not()
        .map(|caps| caps.unwrap_or_default());

    super::block_label()
        .then(ports)
        .then(captures)
        .map_with(|((label, ports), captures), e| Spanned {
            value: GraphHeader {
                name: label.name,
                ports,
                captures,
            },
            span: e.span(),
        })
        .labelled("graph header")
}

/// Parses a `yield %v0, %v1;` clause.
fn yield_clause<'t, I>() -> impl Parser<'t, I, Vec<Spanned<&'t str>>, ParserError<'t>>
where
    I: TokenInput<'t>,
{
    just(Token::Identifier("yield"))
        .ignore_then(
            ssa_name()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(Token::Semicolon))
        .labelled("yield clause")
}

/// Parses a directed graph body.
///
/// Matches:
/// ```text
/// digraph ^dg0(%p0: Type) capture(%theta: f64) {
///   %0 = constant 1;
///   %1 = add %p0, %0;
///   yield %1;
/// }
/// ```
pub fn digraph<'t, I, T, S>(
    language: RecursiveParser<'t, I, S>,
) -> impl Parser<
    't,
    I,
    DiGraph<'t, <T as HasParser<'t>>::Output, S>,
    ParserError<'t>,
>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
    S: Clone,
{
    let header = just(Token::Identifier("digraph")).ignore_then(graph_header::<_, T>());

    let statements = language
        .clone()
        .map_with(|stmt, e| Spanned {
            value: stmt,
            span: e.span(),
        })
        .then_ignore(just(Token::Semicolon))
        .repeated()
        .collect::<Vec<_>>();

    let yields = yield_clause().or_not().map(|y| y.unwrap_or_default());

    let body = statements
        .then(yields)
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    header
        .then(body)
        .map(|(header, (statements, yields))| DiGraph {
            header,
            statements,
            yields,
        })
        .labelled("digraph")
}

/// Parses an ungraph statement — either `edge <stmt>` or plain `<stmt>`.
fn ungraph_statement<'t, I, S>(
    language: RecursiveParser<'t, I, S>,
) -> impl Parser<'t, I, UnGraphStatement<'t, S>, ParserError<'t>>
where
    I: TokenInput<'t>,
    S: Clone,
{
    let edge_prefix = just(Token::Identifier("edge"))
        .map_with(|_, e| e.span())
        .or_not();

    edge_prefix
        .then(language.map_with(|stmt, e| Spanned {
            value: stmt,
            span: e.span(),
        }))
        .then_ignore(just(Token::Semicolon))
        .map(|(edge_span, stmt)| {
            UnGraphStatement::new(edge_span.is_some(), stmt, edge_span)
        })
        .labelled("ungraph statement")
}

/// Parses an undirected graph body.
///
/// Matches:
/// ```text
/// ungraph ^ug0(%p0: Type) capture(%theta: f64) {
///   edge %w0 = wire;
///   node_a(%p0, %w0);
///   node_b(%theta, %w0);
/// }
/// ```
pub fn ungraph<'t, I, T, S>(
    language: RecursiveParser<'t, I, S>,
) -> impl Parser<
    't,
    I,
    UnGraph<'t, <T as HasParser<'t>>::Output, S>,
    ParserError<'t>,
>
where
    I: TokenInput<'t>,
    T: HasParser<'t>,
    S: Clone,
{
    let header = just(Token::Identifier("ungraph")).ignore_then(graph_header::<_, T>());

    let body = ungraph_statement(language)
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    header
        .then(body)
        .map(|(header, statements)| UnGraph {
            header,
            statements,
        })
        .labelled("ungraph")
}
