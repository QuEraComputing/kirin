//! Format string parsing.
//!
//! Format strings define the syntax for parsing dialect statements.
//! For example: `"{res} = add {lhs} {rhs}"` defines a statement that
//! parses a result value, an equals sign, the keyword "add", and two
//! operands.
//!
//! # Escaping
//!
//! To include a literal `{` character in the format string, use `{{`:
//! - `"{{"` produces a literal `{` token
//!
//! Note: `}` characters don't need escaping since they're only special
//! when closing an interpolation. Use `}` directly in the format string.

use chumsky::input::Stream;
use chumsky::prelude::*;
use chumsky::span::SimpleSpan;
use kirin_lexer::{Logos, Token};
use proc_macro2::Span;

/// A parsed format string.
#[derive(Debug, Clone, Default)]
pub struct Format<'src> {
    elements: Vec<FormatElement<'src>>,
}

/// An element in a format string.
#[derive(Debug, Clone)]
pub enum FormatElement<'src> {
    /// Literal tokens to match exactly.
    Token(Vec<Token<'src>>),
    /// A field interpolation like `{name}`, `{name:type}`, or `{name:ports}`.
    Field(&'src str, FormatOption),
    /// A keyword interpolation like `$add` that gets namespace-prefixed.
    Keyword(&'src str),
    /// A context projection like `{:name}` — properties of the enclosing function.
    Context(ContextProjection),
}

/// Context projections: `{:name}`, `{:...}` — properties of the enclosing function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextProjection {
    /// `{:name}` — the function's global symbol name (`@symbol`).
    Name,
}

/// Projections for `{field:...}` body structural parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyProjection {
    /// `{field:ports}` — graph port declarations (`%name: Type, ...`).
    Ports,
    /// `{field:captures}` — graph capture declarations (`%name: Type, ...`).
    Captures,
    /// `{field:yields}` — yield types (`Type, Type`).
    Yields,
    /// `{field:args}` — block arguments (`%name: Type, ...`).
    Args,
    /// `{field:body}` — inner statements only (no header, no braces).
    Body,
}

/// Options for field interpolation.
#[derive(Debug, Clone, Default)]
pub enum FormatOption {
    /// Interpolate the field's name (e.g., for SSAValue use its name).
    Name,
    /// Interpolate the field's type (e.g., for SSAValue use its type).
    Type,
    /// Default behavior based on field type.
    #[default]
    Default,
    /// Body structural projection on a field (`{field:ports}`, `{field:body}`, etc.).
    Body(BodyProjection),
}

impl<'src> Format<'src> {
    /// Creates a new format from parsed elements.
    pub fn new(elements: Vec<FormatElement<'src>>) -> Self {
        Self { elements }
    }

    /// Returns all elements in the format string.
    pub fn elements(&self) -> &[FormatElement<'src>] {
        &self.elements
    }

    /// Creates a parser for format strings.
    fn parser<'tokens, I>()
    -> impl Parser<'tokens, I, Format<'src>, extra::Err<Rich<'tokens, Token<'src>, SimpleSpan>>>
    where
        'src: 'tokens,
        I: chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>,
    {
        // Parse escaped braces: {{ -> literal {, }} -> literal }
        // The lexer produces EscapedLBrace/EscapedRBrace tokens for {{ and }}
        let escaped_lbrace =
            just(Token::EscapedLBrace).to(FormatElement::Token(vec![Token::EscapedLBrace]));

        let escaped_rbrace =
            just(Token::EscapedRBrace).to(FormatElement::Token(vec![Token::EscapedRBrace]));

        // Parse dollar keyword like $add
        let dollar_keyword = just(Token::Dollar)
            .ignore_then(select! { Token::Identifier(name) => name })
            .map(FormatElement::Keyword);

        // Parse context projection: {:name} (empty field name = enclosing function property)
        let context_projection = just(Token::LBrace)
            .ignore_then(just(Token::Colon))
            .ignore_then(select! {
                Token::Identifier("name") => ContextProjection::Name,
            })
            .then_ignore(just(Token::RBrace))
            .map(FormatElement::Context);

        // Parse field interpolations: {name}, {name:type}, {name:ports}, {name:body}, etc.
        let interpolation = just(Token::LBrace)
            .ignore_then(
                select! {
                    Token::Identifier(name) => name,
                    Token::Int(name) => name
                }
                .then(
                    just(Token::Colon)
                        .ignore_then(select! {
                            Token::Identifier("type") => FormatOption::Type,
                            Token::Identifier("name") => FormatOption::Name,
                            Token::Identifier("ports") => FormatOption::Body(BodyProjection::Ports),
                            Token::Identifier("captures") => FormatOption::Body(BodyProjection::Captures),
                            Token::Identifier("yields") => FormatOption::Body(BodyProjection::Yields),
                            Token::Identifier("args") => FormatOption::Body(BodyProjection::Args),
                            Token::Identifier("body") => FormatOption::Body(BodyProjection::Body),
                        })
                        .or_not(),
                ),
            )
            .then_ignore(just(Token::RBrace))
            .map(|(name, opt)| FormatElement::Field(name, opt.unwrap_or_default()));

        // Parse literal tokens (anything that's not `{` or escaped braces)
        // Note: Regular `}` is allowed in literal tokens since it's only special after `{`
        let other = any()
            .filter(|t: &Token| {
                !matches!(
                    t,
                    Token::LBrace | Token::EscapedLBrace | Token::EscapedRBrace
                )
            })
            .repeated()
            .at_least(1)
            .collect()
            .map(FormatElement::Token);

        // Order matters: try escaped braces first, then dollar keyword,
        // then context projection ({:name}), then generic interpolation, then other
        escaped_lbrace
            .or(escaped_rbrace)
            .or(dollar_keyword)
            .or(context_projection)
            .or(interpolation)
            .or(other)
            .repeated()
            .collect()
            .map(Format::new)
    }

    /// Parses a format string.
    pub fn parse(input: &'src str, span: Option<Span>) -> syn::Result<Self> {
        // Check for legacy {.keyword} syntax and produce a helpful error
        if input.contains("{.") {
            // Simple heuristic: look for {.identifier} pattern
            let mut chars = input.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '{' {
                    if let Some(&'.') = chars.peek() {
                        return Err(syn::Error::new(
                            span.unwrap_or_else(Span::call_site),
                            "Legacy `{.keyword}` syntax is no longer supported. \
                             Use `$keyword` instead of `{.keyword}` in format strings.",
                        ));
                    }
                }
            }
        }

        let token_iter = Token::lexer(input).spanned().map(|(tok, span)| match tok {
            Ok(tok) => (tok, span.into()),
            Err(()) => (Token::Error, span.into()),
        });
        let token_stream =
            Stream::from_iter(token_iter).map((0..input.len()).into(), |(t, s): (_, _)| (t, s));

        let parser = Self::parser();

        match parser.parse(token_stream).into_result() {
            Ok(fmt) => Ok(fmt),
            Err(errors) => {
                let compile_errors: syn::Error = errors.into_iter().fold(
                    syn::Error::new(span.unwrap_or_else(Span::call_site), "Format parse error"),
                    |mut acc, e: Rich<Token>| {
                        let msg = format!("{} at {:?}", e.reason(), e.span());
                        acc.combine(syn::Error::new(span.unwrap_or_else(Span::call_site), msg));
                        acc
                    },
                );
                Err(compile_errors)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_parser_basic() {
        let input = "load something {value:type} from {address}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_format_parser_complex() {
        let input = "load something {value:name} from {address}: {value:type} -> {result:type}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_format_parser_positional() {
        // New syntax: explicit {field:name} for result name, {field:type} for result type
        let input = "{0:name} = add {1}, {2} -> {0:type}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_legacy_brace_keyword_rejected() {
        let input = "{result:name} = {.add} {lhs}, {rhs} -> {result:type}";
        let err = Format::parse(input, None).unwrap_err();
        assert!(
            err.to_string().contains("$keyword"),
            "Error should suggest $keyword syntax: {}",
            err
        );
    }

    #[test]
    fn test_legacy_brace_keyword_only_rejected() {
        let input = "{.ret} {0}";
        let err = Format::parse(input, None).unwrap_err();
        assert!(
            err.to_string().contains("$keyword"),
            "Error should suggest $keyword syntax: {}",
            err
        );
    }

    #[test]
    fn test_dollar_keyword() {
        let input = "$h {qubit}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_dollar_keyword_with_underscore() {
        let input = "$z_spider({angle}) {legs}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_dollar_keyword_cnot() {
        let input = "$cnot {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_context_name_projection() {
        let input = "fn {:name}({body:ports}) -> {body:yields}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_body_ports_projection() {
        let input = "{body:ports}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_body_body_projection() {
        let input = "{{ {body:body} }}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_body_captures_projection() {
        let input = "fn {:name}({body:ports}) captures ({body:captures}) -> {body:yields} {{ {body:body} }}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_body_args_projection() {
        let input = "fn {:name}({body:args})";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_body_without_projection_is_default() {
        // {body} without :projection should be FormatOption::Default (backward compat)
        let input = "{body}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_format_parser_escaped_lbrace() {
        // Test escaped opening brace: {{ -> literal {
        let input = "dict {{ {key} }} = {value}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }
}
