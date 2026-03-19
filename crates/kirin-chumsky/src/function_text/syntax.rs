use chumsky::input::Stream;
use chumsky::prelude::*;
use kirin_ir::{Dialect, Signature};
use kirin_lexer::{Logos, Token};

use crate::ast::SymbolName;
use crate::parsers::{identifier, symbol};
use crate::traits::{HasParser, ParserError, TokenInput};

pub(super) type ChumskyError<'src> = Rich<'src, Token<'src>, SimpleSpan>;

#[derive(Debug, Clone)]
pub(super) struct Header<'src, T> {
    pub stage: SymbolName<'src>,
    pub function: SymbolName<'src>,
    pub signature: Signature<T>,
    pub span: SimpleSpan,
}

#[derive(Debug, Clone)]
pub(super) enum Declaration<'src, T> {
    Stage(Header<'src, T>),
    Specialize {
        header: Header<'src, T>,
        /// Span of the body portion only (the brace-balanced region).
        body_span: SimpleSpan,
        /// Span of the entire specialize declaration.
        span: SimpleSpan,
    },
}

#[derive(Debug, Clone)]
struct ParsedFnSignature<'src, T> {
    function: SymbolName<'src>,
    signature: Signature<T>,
}

fn type_list_parser<'src, I, L>() -> impl Parser<'src, I, Vec<L::Type>, ParserError<'src>>
where
    I: TokenInput<'src>,
    L: Dialect + HasParser<'src>,
    L::Type: HasParser<'src, Output = L::Type>,
{
    L::Type::parser()
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .labelled("type list")
}

fn fn_signature_parser<'src, I, L>()
-> impl Parser<'src, I, ParsedFnSignature<'src, L::Type>, ParserError<'src>>
where
    I: TokenInput<'src>,
    L: Dialect + HasParser<'src>,
    L::Type: HasParser<'src, Output = L::Type>,
{
    identifier("fn")
        .ignore_then(symbol())
        .then(type_list_parser::<I, L>())
        .then_ignore(just(Token::Arrow))
        .then(L::Type::parser())
        .map(|((function, params), ret)| ParsedFnSignature {
            function,
            signature: Signature::new(params, ret, ()),
        })
        .labelled("function signature")
}

/// Brace-balanced body scanner. Matches `{ ... }` with arbitrary nesting,
/// returning only the span. Does not parse the body contents.
fn brace_body_span<'src, I>() -> impl Parser<'src, I, SimpleSpan, ParserError<'src>>
where
    I: TokenInput<'src>,
{
    chumsky::primitive::custom(|input: &mut chumsky::input::InputRef<'src, '_, I, _>| {
        let start = input.cursor();
        match input.next() {
            Some(Token::LBrace) => {}
            Some(found) => {
                return Err(Rich::custom(
                    input.span_since(&start),
                    format!("expected '{{', found {found}"),
                ));
            }
            None => return Err(Rich::custom(input.span_since(&start), "expected '{'")),
        }
        let mut depth: u32 = 1;
        while depth > 0 {
            match input.next() {
                Some(Token::LBrace) => depth += 1,
                Some(Token::RBrace) => depth -= 1,
                Some(_) => {}
                None => return Err(Rich::custom(input.span_since(&start), "unclosed '{'")),
            }
        }
        Ok(input.span_since(&start))
    })
}

fn declaration_parser<'src, I, L>()
-> impl Parser<'src, I, Declaration<'src, L::Type>, ParserError<'src>>
where
    I: TokenInput<'src>,
    L: Dialect + HasParser<'src>,
    L::Type: HasParser<'src, Output = L::Type>,
{
    let stage_decl = identifier("stage")
        .ignore_then(symbol())
        .then(fn_signature_parser::<I, L>())
        .then_ignore(just(Token::Semicolon))
        .map_with(|(stage, sig), extra| {
            Declaration::Stage(Header {
                stage,
                function: sig.function,
                signature: sig.signature,
                span: extra.span(),
            })
        });

    let specialize_decl = identifier("specialize")
        .ignore_then(symbol())
        .then(fn_signature_parser::<I, L>())
        .then(brace_body_span::<I>())
        .map_with(|((stage, sig), body_span), extra| Declaration::Specialize {
            header: Header {
                stage,
                function: sig.function,
                signature: sig.signature,
                span: extra.span(),
            },
            body_span,
            span: extra.span(),
        });

    choice((stage_decl, specialize_decl))
}

pub(super) fn tokenize<'src>(src: &'src str) -> Vec<(Token<'src>, SimpleSpan)> {
    Token::lexer(src)
        .spanned()
        .map(|(token, span)| (token.unwrap_or(Token::Error), SimpleSpan::from(span)))
        .collect()
}

pub(super) fn parse_one_declaration<'src, L>(
    tokens: &[(Token<'src>, SimpleSpan)],
) -> Result<(Declaration<'src, L::Type>, SimpleSpan), Vec<ChumskyError<'src>>>
where
    L: Dialect + HasParser<'src>,
    L::Type: HasParser<'src, Output = L::Type>,
{
    let end = tokens.last().map(|(_, span)| span.end).unwrap_or_default();
    let eoi = SimpleSpan::from(end..end);
    let stream = Stream::from_iter(tokens.to_vec()).map(eoi, |(token, span)| (token, span));

    declaration_parser::<_, L>()
        .map_with(|declaration, extra| (declaration, extra.span()))
        .then_ignore(any().repeated())
        .parse(stream)
        .into_result()
}
