use chumsky::input::Stream;
use chumsky::prelude::*;
use kirin_ir::{Dialect, Signature};
use kirin_lexer::{Logos, Token};

use crate::ast::SymbolName;
use crate::parsers::{identifier, symbol};
use crate::traits::{HasParser, ParserError, TokenInput};

pub(super) type RichError<'src> = Rich<'src, Token<'src>, SimpleSpan>;

#[derive(Debug, Clone)]
pub(super) struct Header<'src, T> {
    #[allow(dead_code)]
    pub stage: SymbolName<'src>,
    #[allow(dead_code)]
    pub function: SymbolName<'src>,
    pub signature: Signature<T>,
    pub span: SimpleSpan,
}

#[derive(Debug, Clone)]
pub(super) enum Declaration<'src, T> {
    Stage(Header<'src, T>),
    Specialize {
        stage: SymbolName<'src>,
        /// Span of the body portion (from keyword through closing `}`).
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

/// Body span scanner. Skips the body's discriminator and header — whatever the
/// dialect's format string puts before the first `{` — then matches a
/// brace-balanced `{ ... }`. Returns the span covering everything from the
/// first token through the matching closing brace. Does not parse body
/// contents; that is the dialect statement parser's job, which is what keeps
/// dialect-level validation intact.
///
/// All four body kinds carry an explicit textual discriminator, and each is
/// scanned by the same rule:
///
/// ```text
/// fn @f(..) -> T cfg { ^entry(..) { .. } }      // keyword, then the CFG's braces
/// fn @f(..) -> T block ^body(..) { .. }         // keyword + header, then braces
/// fn @f(..) -> T digraph ^g0(..) { .. }         // keyword + header, then braces
/// fn @f(..) -> T ungraph ^u0(..) { .. }         // keyword + header, then braces
/// ```
///
/// Projected formats (`fn @f(..) -> T (%x: T) { .. }`) work the same way: the
/// scanner does not care what the prefix tokens are, only where the first `{`
/// is.
fn body_span<'src, I>() -> impl Parser<'src, I, SimpleSpan, ParserError<'src>>
where
    I: TokenInput<'src>,
{
    chumsky::primitive::custom(|input: &mut chumsky::input::InputRef<'src, '_, I, _>| {
        let start = input.cursor();
        // Skip tokens until we find the opening brace. This is what lets the
        // discriminator and header through: `cfg {`, `block ^name(args...) {`,
        // `digraph ^name(ports...) {`, `ungraph ^name(...) {`.
        loop {
            match input.next() {
                Some(Token::LBrace) => break,
                Some(_) => {}
                None => {
                    return Err(Rich::custom(
                        input.span_since(&start),
                        "expected '{' in body",
                    ));
                }
            }
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

    // Unified specialize path: framework only strips `specialize @stage`.
    // The full remaining text (from keyword through closing `}`) is passed
    // to the dialect's statement parser, which handles {:name}, {sig}, {body}, etc.
    // The function name is extracted post-parse from EmitContext::function_name().
    let specialize_decl = identifier("specialize")
        .ignore_then(symbol())
        .then(body_span::<I>()) // captures from keyword (e.g. `fn`) through closing `}`
        .map_with(|(stage, body_span), extra| Declaration::Specialize {
            stage,
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
) -> Result<(Declaration<'src, L::Type>, SimpleSpan), Vec<RichError<'src>>>
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
