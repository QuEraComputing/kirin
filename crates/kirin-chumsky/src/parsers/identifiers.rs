use crate::ast::*;
use crate::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

/// Parses a specific identifier keyword.
///
/// # Example
///
/// ```ignore
/// let add_kw = identifier("add"); // matches "add" exactly
/// ```
pub fn identifier<'t, I>(name: &'t str) -> impl Parser<'t, I, Spanned<&'t str>, ParserError<'t>>
where
    I: TokenInput<'t>,
{
    select! { Token::Identifier(id) = e if id == name => Spanned {
        value: id,
        span: e.span(),
    }}
    .labelled(format!("identifier '{}'", name))
}

/// Parses any identifier.
pub fn any_identifier<'t, I>() -> impl Parser<'t, I, Spanned<&'t str>, ParserError<'t>>
where
    I: TokenInput<'t>,
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
/// let sym = symbol(); // matches "@foo", returns SymbolName { name: "foo", span: ... }
/// ```
pub fn symbol<'t, I>() -> impl Parser<'t, I, SymbolName<'t>, ParserError<'t>>
where
    I: TokenInput<'t>,
{
    select! { Token::Symbol(sym) = e => SymbolName {
        name: sym,
        span: e.span(),
    }}
    .labelled("symbol")
}
