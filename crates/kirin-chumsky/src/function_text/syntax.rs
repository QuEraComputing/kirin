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
        function: SymbolName<'src>,
        /// Framework-parsed signature from `fn @name(types) -> type`.
        /// `None` when the dialect controls the format (e.g., projection-based).
        signature: Option<Signature<T>>,
        /// Span of the body portion (from after `fn @name` through closing `}`).
        /// When signature is Some, this covers `keyword { ... }` or `{ ... }`.
        /// When signature is None, this covers `(%q: T) -> T { ... }`.
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

/// Parses just `(types) -> type` — the parameter/return part of a function signature.
fn fn_params_and_return<'src, I, L>()
-> impl Parser<'src, I, Signature<L::Type>, ParserError<'src>>
where
    I: TokenInput<'src>,
    L: Dialect + HasParser<'src>,
    L::Type: HasParser<'src, Output = L::Type>,
{
    type_list_parser::<I, L>()
        .then_ignore(just(Token::Arrow))
        .then(L::Type::parser())
        .map(|(params, ret)| Signature::new(params, ret, ()))
        .labelled("function params and return type")
}

/// Body span scanner. Matches an optional keyword prefix (e.g. `digraph`,
/// `ungraph`) followed by a brace-balanced `{ ... }` region. Returns the
/// span covering everything from the first non-brace token (or the opening
/// brace) through the matching closing brace. Does not parse body contents.
fn body_span<'src, I>() -> impl Parser<'src, I, SimpleSpan, ParserError<'src>>
where
    I: TokenInput<'src>,
{
    chumsky::primitive::custom(|input: &mut chumsky::input::InputRef<'src, '_, I, _>| {
        let start = input.cursor();
        // Skip tokens until we find the opening brace. This allows keyword
        // prefixes like `digraph ^name(ports...) {` or `ungraph ^name(...) {`.
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

    // Option A: specialize @stage fn @name(types) -> type { body }
    // Framework parses the signature. body_span starts after the signature.
    let specialize_with_sig = identifier("specialize")
        .ignore_then(symbol())
        .then(fn_signature_parser::<I, L>())
        .then(body_span::<I>())
        .map_with(|((stage, sig), body_span), extra| Declaration::Specialize {
            stage,
            function: sig.function,
            signature: Some(sig.signature),
            body_span,
            span: extra.span(),
        });

    // Option B: specialize @stage fn @name(...dialect format...)
    // Framework can't parse the signature. body_span starts at `fn` to include
    // `fn @name(...)` in the body_text for the dialect parser.
    let specialize_dialect = identifier("specialize")
        .ignore_then(symbol())
        .then(body_span::<I>())  // captures from `fn` through closing `}`
        .map_with(|(stage, body_span), extra| {
            Declaration::Specialize {
                stage,
                // Function name will be extracted from body_text in second_pass
                function: SymbolName { name: "", span: extra.span() },
                signature: None,
                body_span,
                span: extra.span(),
            }
        });

    let specialize_decl = specialize_with_sig.or(specialize_dialect);

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
