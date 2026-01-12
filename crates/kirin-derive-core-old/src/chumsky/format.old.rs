use std::collections::BTreeMap;

use chumsky::prelude::*;
use chumsky::{input::Stream, span::SimpleSpan};
use kirin_lexer::{Logos, Token};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

use crate::derive::Compile;
use crate::gadgets::CratePath;
use crate::ir::{DefinitionStructOrVariant, Fields, SourceIdent};
use crate::kirin::extra::FieldKind;

use super::parser::DeriveHasParser;

#[derive(Debug, Clone, Default)]
pub struct Format<'src> {
    elements: Vec<FormatElement<'src>>,
}

#[derive(Debug, Clone)]
pub enum FormatElement<'src> {
    Token(Vec<Token<'src>>),
    Field(&'src str, Option<FormatOption>),
}

#[derive(Debug, Clone)]
pub enum FormatOption {
    /// interpolate the field's type, e.g if `SSAValue` use its type
    /// error if the field has no associated type
    Type,
}

impl<'src> Format<'src> {
    pub fn new(elements: Vec<FormatElement<'src>>) -> Self {
        Self { elements }
    }

    pub fn elements(&self) -> &Vec<FormatElement<'src>> {
        &self.elements
    }

    pub fn parser<'tokens, I>()
    -> impl Parser<'tokens, I, Format<'src>, extra::Err<Rich<'tokens, Token<'src>, SimpleSpan>>>
    where
        'src: 'tokens,
        I: chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>,
    {
        let interpolation = just(Token::LBrace)
            .ignore_then(
                select! { Token::Identifier(name) => name }.then(
                    just(Token::Colon)
                        .ignore_then(select! { Token::Identifier("type") => FormatOption::Type })
                        .or_not(),
                ),
            )
            .then_ignore(just(Token::RBrace))
            .map(|(name, opt)| FormatElement::Field(name, opt));
        let other = any()
            .filter(|t: &Token| *t != Token::LBrace)
            .repeated()
            .at_least(1)
            .collect()
            .map(|tokens| FormatElement::Token(tokens));

        other
            .or(interpolation)
            .repeated()
            .collect()
            .map(|elems| Format::new(elems))
    }

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

pub struct FormatTokens<'a, 'src> {
    format: Format<'src>,
    fields: Fields<'a, 'src, DeriveHasParser>,
}

pub struct ParseElements {
    steps: Vec<ParseStep>,
    error: Option<syn::Error>,
}

impl ParseElements {
    pub fn new(steps: Vec<ParseStep>) -> Self {
        Self { steps, error: None }
    }

    pub fn error<T: std::fmt::Display>(msg: T) -> Self {
        Self {
            steps: vec![],
            error: Some(syn::Error::new(Span::call_site(), msg)),
        }
    }
}

pub enum ParseStep {
    Ignore(TokenStream),
    Keep(TokenStream),
    Error(syn::Error),
}

impl ParseStep {
    pub fn error(msg: impl std::fmt::Display) -> Self {
        Self::Error(syn::Error::new(Span::call_site(), msg))
    }
}

impl<'a, 'src> Compile<'src, DeriveHasParser, ParseElements> for Fields<'a, 'src, DeriveHasParser> {
    fn compile(&self, ctx: &DeriveHasParser) -> ParseElements {
        let crate_path: CratePath = self.compile(ctx);
        let fields_map =
            BTreeMap::from_iter(self.iter().map(|f| (f.source_ident().to_string(), f)));

        let Some(format) = (match self.definition() {
            DefinitionStructOrVariant::Struct(data) => data.attrs.format,
            DefinitionStructOrVariant::Variant(e, i) => e.variants[*i].attrs.format,
        }) else {
            return ParseElements::error("No format specified");
        };

        let Ok(format) = Format::parse(&format, None) else {
            return ParseElements::error("Failed to parse format");
        };

        let steps: Vec<ParseStep> = format
            .elements
            .iter()
            .map(|elem| match elem {
                FormatElement::Token(tokens) => {
                    let mut iter = tokens.iter();
                    let Some(first_token) = iter.next() else {
                        let msg = "Format string cannot be empty";
                        return ParseStep::error(msg);
                    };
                    let mut expr = quote! { just(#first_token) };
                    for t in iter {
                        expr = quote! { #expr.then(just(#t)) };
                    }
                    ParseStep::Ignore(expr)
                }
                FormatElement::Field(name, _opt) => {
                    let Some(f) = fields_map.get(*name) else {
                        return ParseStep::error(format!("Field '{}' not found for format", name));
                    };

                    let expr = match f.extra().kind {
                        FieldKind::SSAValue => quote! { #crate_path::operand() },
                        FieldKind::Block => quote! { #crate_path::block() },
                        FieldKind::Successor => quote! { #crate_path::successor() },
                        _ => {
                            return ParseStep::error(format!(
                                "Field '{}' cannot be used in format",
                                name
                            ));
                        }
                    };
                    ParseStep::Keep(expr)
                }
            })
            .collect();
        ParseElements::new(steps)
    }
}

impl<'a, 'src> Compile<'src, DeriveHasParser, FormatTokens<'a, 'src>>
    for Fields<'a, 'src, DeriveHasParser>
{
    fn compile(&self, ctx: &DeriveHasParser) -> FormatTokens<'a, 'src> {}
}
