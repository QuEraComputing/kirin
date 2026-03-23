//! Format string parsing.
//!
//! Format strings define the syntax for parsing and printing dialect statements.
//! They are specified via the `#[chumsky(format = "...")]` attribute on dialect
//! structs or enum variants.
//!
//! # EBNF Grammar
//!
//! ```text
//! format         ::= element*
//! element        ::= escaped_brace | escaped_bracket | optional_section
//!                   | dollar_keyword | context_proj | interpolation | literal+
//!
//! escaped_brace  ::= '{{' | '}}'
//! escaped_bracket::= '[[' | ']]'
//! optional_section ::= '[' element* ']'
//! dollar_keyword ::= '$' IDENT
//! context_proj   ::= '{:' context_name '}'
//! interpolation  ::= '{' field_ref (':' projection)? '}'
//!
//! field_ref      ::= IDENT | INT
//! projection     ::= 'name' | 'type' | body_proj | sig_proj
//! body_proj      ::= 'ports' | 'captures' | 'args' | 'body'
//! sig_proj       ::= 'inputs' | 'return'
//! context_name   ::= 'name'
//!
//! literal        ::= <any token except '{', '{{', '}}', '[', '[[', ']]', ']'>
//! IDENT          ::= <identifier token>
//! INT            ::= <integer literal token>
//! ```
//!
//! # Projection Table
//!
//! Not all projections are valid on every field category. The table below shows
//! which projections are accepted for each [`FieldCategory`](kirin_derive_toolkit::ir::fields::FieldCategory).
//!
//! | Field Category | `(default)` | `:name` | `:type` | `:ports` | `:captures` | `:args` | `:body` | `:inputs` | `:return` |
//! |----------------|:-----------:|:-------:|:-------:|:--------:|:-----------:|:-------:|:-------:|:---------:|:---------:|
//! | Argument       | yes         | yes     | yes     |          |             |         |         |           |           |
//! | Result         | --          | --      | yes     |          |             |         |         |           |           |
//! | Block          | yes         |         |         |          |             | yes     | yes     |           |           |
//! | Successor      | yes         |         |         |          |             |         |         |           |           |
//! | Region         | yes         |         |         |          |             |         | yes     |           |           |
//! | Symbol         | yes         |         |         |          |             |         |         |           |           |
//! | Value          | yes         |         |         |          |             |         |         |           |           |
//! | DiGraph        | yes         |         |         | yes      | yes         |         | yes     |           |           |
//! | UnGraph        | yes         |         |         | yes      | yes         |         | yes     |           |           |
//! | Signature      | yes         |         |         |          |             |         |         | yes       | yes       |
//!
//! **Result fields**: Result names (`%name =`) are parsed generically by the
//! framework. Only `:type` is valid in the format string. Using `{result}` or
//! `{result:name}` is rejected by validation.
//!
//! **Body projection completeness**: When any body projection (`:ports`, `:captures`,
//! `:args`, `:body`) is used on a field, all required projections for that field
//! category must be present for roundtrip correctness. For example, a DiGraph field
//! with `:body` must also have `:ports` and `:captures`.
//!
//! # Escaping
//!
//! To include a literal `{` character in the format string, use `{{`:
//! - `"{{"` produces a literal `{` token
//!
//! To include a literal `[` or `]` character in the format string, use `[[` or `]]`:
//! - `"[["` produces a literal `[` token
//! - `"]]"` produces a literal `]` token
//!
//! Note: `}` characters don't need escaping since they're only special
//! when closing an interpolation. Use `}` directly in the format string.
//!
//! # Examples
//!
//! ```text
//! // Simple binary operation with keyword, two arguments, and result type:
//! "$add {lhs}, {rhs} -> {result:type}"
//!
//! // Quantum gate with multiple results:
//! "$cnot {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}"
//!
//! // Function body with signature projections and context name:
//! "fn {:name}({sig:inputs}) -> {sig:return} ({body:ports}) captures ({body:captures}) {{ {body:body} }}"
//!
//! // Block field with args/body projections:
//! "$for ({body:args}) {{ {body:body} }}"
//!
//! // Literal braces via escaping:
//! "dict {{ {key} }} = {value}"
//! ```

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
    /// An optional section `[...]` — parsed as all-or-nothing.
    /// Fields inside must be `Option<T>` or `Vec<T>`. Nesting is disallowed.
    Optional(Vec<FormatElement<'src>>),
}

/// Context projections: `{:name}` — properties of the enclosing function.
///
/// After RFC 0004, only `{:name}` remains. `{:return}` and `{:signature}` have been
/// replaced by Signature field projections (`{sig:inputs}`, `{sig:return}`).
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
    /// `{field:args}` — block arguments (`%name: Type, ...`).
    Args,
    /// `{field:body}` — inner statements only (no header, no braces).
    Body,
}

/// Projections for `{sig:...}` Signature field parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureProjection {
    /// `{sig:inputs}` — comma-separated input type list (`Type, Type`).
    Inputs,
    /// `{sig:return}` — single return type (`Type`).
    Return,
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
    /// Signature projection on a Signature field (`{sig:inputs}`, `{sig:return}`).
    Signature(SignatureProjection),
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

        // Parse escaped brackets: [[ -> literal [, ]] -> literal ]
        let escaped_lbracket =
            just(Token::EscapedLBracket).to(FormatElement::Token(vec![Token::EscapedLBracket]));

        let escaped_rbracket =
            just(Token::EscapedRBracket).to(FormatElement::Token(vec![Token::EscapedRBracket]));

        // Parse dollar keyword like $add
        let dollar_keyword = just(Token::Dollar)
            .ignore_then(select! { Token::Identifier(name) => name })
            .map(FormatElement::Keyword);

        // Parse context projection: {:name} (enclosing function name)
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
                            Token::Identifier("args") => FormatOption::Body(BodyProjection::Args),
                            Token::Identifier("body") => FormatOption::Body(BodyProjection::Body),
                            Token::Identifier("inputs") => FormatOption::Signature(SignatureProjection::Inputs),
                            Token::Identifier("return") => FormatOption::Signature(SignatureProjection::Return),
                        })
                        .or_not(),
                ),
            )
            .then_ignore(just(Token::RBrace))
            .map(|(name, opt)| FormatElement::Field(name, opt.unwrap_or_default()));

        // Parse literal tokens (anything that's not special)
        // Note: Regular `}` is allowed in literal tokens since it's only special after `{`
        let other = any()
            .filter(|t: &Token| {
                !matches!(
                    t,
                    Token::LBrace
                        | Token::EscapedLBrace
                        | Token::EscapedRBrace
                        | Token::LBracket
                        | Token::EscapedLBracket
                        | Token::EscapedRBracket
                        | Token::RBracket
                )
            })
            .repeated()
            .at_least(1)
            .collect()
            .map(FormatElement::Token);

        // Inner element parser (used for both top-level and inside [...])
        let inner_element = escaped_lbrace
            .or(escaped_rbrace)
            .or(escaped_lbracket)
            .or(escaped_rbracket)
            .or(dollar_keyword)
            .or(context_projection)
            .or(interpolation)
            .or(other);

        // Parse optional section: [...] — no nesting allowed
        // Inside an optional section, we parse all inner elements except [ and ]
        // (nesting is disallowed by validation, but we also don't parse [ inside)
        let optional_section = just(Token::LBracket)
            .ignore_then(inner_element.clone().repeated().collect::<Vec<_>>())
            .then_ignore(just(Token::RBracket))
            .map(FormatElement::Optional);

        // Order matters: try escaped braces/brackets first, then optional section,
        // then dollar keyword, then context projection ({:name}), then generic interpolation, then other
        inner_element
            .or(optional_section)
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
                if c == '{' && chars.peek() == Some(&'.') {
                    return Err(syn::Error::new(
                        span.unwrap_or_else(Span::call_site),
                        "Legacy `{.keyword}` syntax is no longer supported. \
                         Use `$keyword` instead of `{.keyword}` in format strings.",
                    ));
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
        let input = "fn {:name}({body:ports})";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_signature_projections() {
        let input = "fn {:name}({sig:inputs}) -> {sig:return} {body}";
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
        let input = "fn {:name}({sig:inputs}) -> {sig:return} ({body:ports}) captures ({body:captures}) {{ {body:body} }}";
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
    fn test_optional_section() {
        let input = "$if {condition} then {then_body} else {else_body}[ -> {result:type}]";
        let format = Format::parse(input, None).expect("Failed to parse format");
        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_optional_section_call() {
        let input = "$call {target}({args})[ -> {results:type}]";
        let format = Format::parse(input, None).expect("Failed to parse format");
        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_optional_section_escaped_brackets() {
        // [[ and ]] produce literal bracket tokens
        let input = "$for {iv} in {start}..{end} [[ step {step} ]]";
        let format = Format::parse(input, None).expect("Failed to parse format");
        insta::assert_debug_snapshot!(format);
    }

    #[test]
    fn test_optional_section_multiple() {
        let input = "$op {x}[ -> {a:type}][ -> {b:type}]";
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
