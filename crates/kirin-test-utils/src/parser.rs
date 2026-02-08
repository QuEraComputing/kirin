use kirin_chumsky::HasParser;
use kirin_chumsky::chumsky::input::{Input, Stream, ValueInput};
use kirin_chumsky::chumsky::span::SimpleSpan;
use kirin_lexer::{Logos, Token};

pub use kirin_chumsky::chumsky::Parser;

pub type TokenWithSpan<'src> = (Token<'src>, SimpleSpan);

pub fn tokenize<'src>(input: &'src str) -> Vec<TokenWithSpan<'src>> {
    Token::lexer(input)
        .spanned()
        .map(|(token, span)| (token.unwrap_or(Token::Error), SimpleSpan::from(span)))
        .collect()
}

pub fn token_stream<'src>(
    input: &'src str,
) -> impl ValueInput<'src, Token = Token<'src>, Span = SimpleSpan> {
    let tokens = tokenize(input);
    let eoi = SimpleSpan::from(input.len()..input.len());
    Stream::from_iter(tokens).map(eoi, |(token, span)| (token, span))
}

pub fn parse_has_parser<'src, T>(input: &'src str) -> Result<T::Output, Vec<String>>
where
    T: HasParser<'src, 'src>,
{
    use kirin_chumsky::chumsky::Parser;

    let result = T::parser().parse(token_stream(input));
    match result.into_result() {
        Ok(output) => Ok(output),
        Err(errors) => Err(errors.into_iter().map(|error| error.to_string()).collect()),
    }
}
