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
use indexmap::IndexMap;
use kirin_lexer::{Logos, Token};
use proc_macro2::Span;

/// A parsed format string.
#[derive(Debug, Clone, Default)]
pub struct Format<'src> {
    elements: Vec<FormatElement<'src>>,
    fields: IndexMap<&'src str, usize>,
}

/// An element in a format string.
#[derive(Debug, Clone)]
pub enum FormatElement<'src> {
    /// Literal tokens to match exactly.
    Token(Vec<Token<'src>>),
    /// A field interpolation like `{name}` or `{name:type}`.
    Field(&'src str, FormatOption),
}

/// Options for field interpolation.
#[derive(Debug, Clone)]
pub enum FormatOption {
    /// Interpolate the field's name (e.g., for SSAValue use its name).
    Name,
    /// Interpolate the field's type (e.g., for SSAValue use its type).
    Type,
    /// Default behavior based on field type.
    Default,
}

impl Default for FormatOption {
    fn default() -> Self {
        FormatOption::Default
    }
}

impl<'src> Format<'src> {
    /// Creates a new format from parsed elements.
    pub fn new(elements: Vec<FormatElement<'src>>) -> Self {
        let mut fields = IndexMap::new();
        for elem in &elements {
            if let FormatElement::Field(name, _) = elem {
                let len = fields.len();
                fields.entry(*name).or_insert(len);
            }
        }
        Self { elements, fields }
    }

    /// Returns the index of a field by name.
    pub fn get_field_index(&self, name: &str) -> Option<usize> {
        self.fields.get(name).copied()
    }

    /// Returns the format option for a field by name.
    pub fn get_field(&self, name: &str) -> Option<FormatOption> {
        self.fields
            .get(name)
            .map(|idx| self.elements[*idx].clone())
            .and_then(|elem| match elem {
                FormatElement::Field(_, opt) => Some(opt),
                _ => None,
            })
    }

    /// Returns all elements in the format string.
    pub fn elements(&self) -> &[FormatElement<'src>] {
        &self.elements
    }

    /// Returns true if the format has any elements.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
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

        // Parse field interpolations like {name} or {name:type}
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

        // Order matters: try escaped braces first, then interpolation, then other
        escaped_lbrace
            .or(escaped_rbrace)
            .or(interpolation)
            .or(other)
            .repeated()
            .collect()
            .map(Format::new)
    }

    /// Parses a format string.
    ///
    /// # Arguments
    ///
    /// * `input` - The format string to parse
    /// * `span` - Optional span for error reporting
    ///
    /// # Returns
    ///
    /// A parsed `Format` or a syn error.
    pub fn parse(input: &'src str, span: Option<Span>) -> syn::Result<Self> {
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
    fn test_format_parser_escaped_lbrace() {
        // Test escaped opening brace: {{ -> literal {
        let input = "dict {{ {key} }} = {value}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }
}
