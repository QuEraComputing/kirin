use chumsky::input::Stream;
use chumsky::prelude::*;
use kirin_lexer::{Logos, Token};

use super::BoxedParser;

/// An alias for token input types used in Kirin Chumsky parsers.
pub trait TokenInput<'t>:
    chumsky::input::ValueInput<'t, Token = Token<'t>, Span = SimpleSpan>
{
}

impl<'t, I> TokenInput<'t> for I where
    I: chumsky::input::ValueInput<'t, Token = Token<'t>, Span = SimpleSpan>
{
}

/// Trait for types that have an associated parser (non-recursive).
///
/// This is used for simple types like type lattices that don't need
/// recursive parsing.
pub trait HasParser<'t> {
    /// The output type of the parser.
    type Output: Clone + PartialEq;

    /// Returns a parser for this type.
    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>;
}

/// Trait for dialect types that can be parsed with chumsky.
///
/// This trait provides recursive parsing capabilities for dialects.
/// The AST type is parameterized by `TypeOutput` (for type annotations) and
/// `LanguageOutput` (for nested statements in blocks/regions).
///
/// Using explicit type parameters instead of GAT projections avoids infinite
/// compilation times when the Language type is self-referential.
///
/// Note: This trait is implemented by the original dialect type (e.g., `SimpleLang`).
pub trait HasDialectParser<'t>: Sized {
    /// The AST type produced by parsing this dialect.
    ///
    /// - `TypeOutput`: The parsed representation of type annotations
    /// - `LanguageOutput`: The AST type for statements in blocks/regions
    type Output<TypeOutput, LanguageOutput>: Clone + PartialEq
    where
        TypeOutput: Clone + PartialEq + 't,
        LanguageOutput: Clone + PartialEq + 't;

    /// Returns a recursive parser for this dialect.
    ///
    /// The `language` parameter is a recursive parser handle that can be used
    /// to parse nested language constructs (like statements within blocks).
    ///
    /// - `TypeOutput`: The parsed type representation (e.g., from type lattice)
    /// - `LanguageOutput`: The outer language's AST type for recursive parsing
    ///
    /// This is a convenience method that delegates to [`namespaced_parser`](Self::namespaced_parser)
    /// with an empty namespace.
    fn recursive_parser<I, TypeOutput, LanguageOutput>(
        language: super::RecursiveParser<'t, I, LanguageOutput>,
    ) -> BoxedParser<'t, I, Self::Output<TypeOutput, LanguageOutput>>
    where
        I: TokenInput<'t>,
        TypeOutput: Clone + PartialEq + 't,
        LanguageOutput: Clone + PartialEq + 't,
    {
        Self::namespaced_parser::<I, TypeOutput, LanguageOutput>(language, &[])
    }

    /// Returns a recursive parser for this dialect, filtered by namespace.
    ///
    /// When `namespace` is non-empty, only statements whose keyword matches
    /// one of the namespace prefixes are parsed. An empty namespace means
    /// all statements are eligible (no filtering).
    fn namespaced_parser<I, TypeOutput, LanguageOutput>(
        language: super::RecursiveParser<'t, I, LanguageOutput>,
        namespace: &[&'static str],
    ) -> BoxedParser<'t, I, Self::Output<TypeOutput, LanguageOutput>>
    where
        I: TokenInput<'t>,
        TypeOutput: Clone + PartialEq + 't,
        LanguageOutput: Clone + PartialEq + 't;
}

/// A parse error with location information.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: SimpleSpan,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "error at {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// Parses a source string into an AST using the given language's parser.
pub fn parse_ast<'t, L>(input: &'t str) -> Result<L::Output, Vec<ParseError>>
where
    L: HasParser<'t>,
{
    let tokens: Vec<_> = Token::lexer(input)
        .spanned()
        .map(|(tok, span)| {
            let token = tok.unwrap_or(Token::Error);
            (token, SimpleSpan::from(span))
        })
        .collect();

    let eoi = SimpleSpan::from(input.len()..input.len());
    let stream = Stream::from_iter(tokens).map(eoi, |(t, s)| (t, s));
    let result = L::parser().parse(stream);

    match result.into_result() {
        Ok(ast) => Ok(ast),
        Err(errors) => Err(errors
            .into_iter()
            .map(|e| ParseError {
                message: e.to_string(),
                span: *e.span(),
            })
            .collect()),
    }
}
