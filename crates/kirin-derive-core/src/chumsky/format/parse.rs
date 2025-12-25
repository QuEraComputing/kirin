use chumsky::prelude::*;
use chumsky::{input::Stream, span::SimpleSpan};
use kirin_lexer::{Logos, Token};
use proc_macro2::Span;

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

    fn parser<'tokens, I>()
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
