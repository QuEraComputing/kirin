use chumsky::prelude::*;
use chumsky::{input::Stream, span::SimpleSpan};
use indexmap::IndexMap;
use kirin_lexer::{Logos, Token};
use proc_macro2::Span;

#[derive(Debug, Clone, Default)]
pub struct Format<'src> {
    elements: Vec<FormatElement<'src>>,
    fields: IndexMap<&'src str, usize>,
}

#[derive(Debug, Clone)]
pub enum FormatElement<'src> {
    Token(Vec<Token<'src>>),
    Field(&'src str, FormatOption),
}

#[derive(Debug, Clone)]
pub enum FormatOption {
    /// interpolate the field's name, e.g if `SSAValue` use its name
    /// use the default fallback name, e.g `%1` for SSA values, `^bb0` for basic blocks
    /// if no name is provided
    Name,
    /// interpolate the field's type, e.g if `SSAValue` use its type
    /// error if the field has no associated type
    Type,
    /// default option
    Default,
}

impl Default for FormatOption {
    fn default() -> Self {
        FormatOption::Default
    }
}

impl<'src> Format<'src> {
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

    pub fn get_field_index(&self, name: &str) -> Option<usize> {
        self.fields.get(name).copied()
    }

    pub fn get_field(&self, name: &str) -> Option<FormatOption> {
        self.fields
            .get(name)
            .map(|idx| self.elements[*idx].clone())
            .and_then(|elem| match elem {
                FormatElement::Field(_, opt) => Some(opt.clone()),
                _ => None,
            })
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
                        .ignore_then(select! {
                            Token::Identifier("type") => FormatOption::Type,
                            Token::Identifier("name") => FormatOption::Name,
                        })
                        .or_not(),
                ),
            )
            .then_ignore(just(Token::RBrace))
            .map(|(name, opt)| FormatElement::Field(name, opt.unwrap_or_default()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_parser() {
        let input = "load something {value:type} from {address}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);

        let input = "load something {value:name} from {address}: {value:type} -> {result:type}";
        let format = Format::parse(input, None).expect("Failed to parse format");

        insta::assert_debug_snapshot!(format);
    }
}
