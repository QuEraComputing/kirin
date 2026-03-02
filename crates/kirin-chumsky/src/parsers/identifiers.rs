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
/// let sym = symbol(); // matches "@foo", returns SymbolName { name: "foo", span: ... }
/// ```
pub fn symbol<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, SymbolName<'src>, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Symbol(sym) = e => SymbolName {
        name: sym,
        span: e.span(),
    }}
    .labelled("symbol")
}
