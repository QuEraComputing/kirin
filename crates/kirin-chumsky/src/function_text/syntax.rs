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
pub(super) enum Declaration<'src, T, B> {
    Stage(Header<'src, T>),
    Specialize {
        header: Header<'src, T>,
        body: B,
        span: SimpleSpan,
    },
}

#[derive(Debug, Clone)]
struct ParsedFnSignature<'src, T> {
    function: SymbolName<'src>,
    signature: Signature<T>,
}

fn type_list_parser<'src, I, L>() -> impl Parser<'src, I, Vec<L::Type>, ParserError<'src, 'src>>
where
    I: TokenInput<'src, 'src>,
    L: Dialect + HasParser<'src, 'src>,
    L::Type: HasParser<'src, 'src, Output = L::Type>,
{
    L::Type::parser()
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .labelled("type list")
}

fn fn_signature_parser<'src, I, L>()
-> impl Parser<'src, I, ParsedFnSignature<'src, L::Type>, ParserError<'src, 'src>>
where
    I: TokenInput<'src, 'src>,
    L: Dialect + HasParser<'src, 'src>,
    L::Type: HasParser<'src, 'src, Output = L::Type>,
{
    identifier("fn")
        .ignore_then(symbol())
        .then(type_list_parser::<I, L>())
        .then_ignore(just(Token::Arrow))
        .then(L::Type::parser())
        .map(|((function, params), ret)| ParsedFnSignature {
            function,
            signature: Signature {
                params,
                ret,
                constraints: (),
            },
        })
        .labelled("function signature")
}

fn declaration_parser<'src, I, L>() -> impl Parser<
    'src,
    I,
    Declaration<'src, L::Type, <L as HasParser<'src, 'src>>::Output>,
    ParserError<'src, 'src>,
>
where
    I: TokenInput<'src, 'src>,
    L: Dialect + HasParser<'src, 'src>,
    L::Type: HasParser<'src, 'src, Output = L::Type>,
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
        .then(L::parser())
        .map_with(|((stage, sig), body), extra| Declaration::Specialize {
            header: Header {
                stage,
                function: sig.function,
                signature: sig.signature,
                span: extra.span(),
            },
            body,
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
) -> Result<
    (
        Declaration<'src, L::Type, <L as HasParser<'src, 'src>>::Output>,
        SimpleSpan,
    ),
    Vec<ChumskyError<'src>>,
>
where
    L: Dialect + HasParser<'src, 'src>,
    L::Type: HasParser<'src, 'src, Output = L::Type>,
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
