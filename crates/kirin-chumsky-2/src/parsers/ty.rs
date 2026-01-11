use super::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

/// a generic type reference containing
/// the string of the corresponding type, e.g
/// ```ignore
/// type
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleType<'src> {
    name: &'src str,
    span: SimpleSpan,
}

/// a function type reference containing
/// parameter types and return type, e.g
/// ```ignore
/// (param_type1, param_type2) -> return_type
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>> {
    param_types: Vec<<Language::TypeLattice as WithChumskyParser<'tokens, 'src, Language>>::Output>,
    return_type: <Language::TypeLattice as WithChumskyParser<'tokens, 'src, Language>>::Output,
    span: SimpleSpan,
}

pub fn simple_type<'tokens, 'src: 'tokens, I>()
-> impl ChumskyParser<'tokens, 'src, I, SimpleType<'src>>
where
    'src: 'tokens,
    I: super::traits::TokenInput<'tokens, 'src>,
{
    select! { Token::Identifier(name) => name }
        .map_with(|name, extra| SimpleType {
            name,
            span: extra.span(),
        })
        .labelled("simple type")
}

pub fn function_type<'tokens, 'src: 'tokens, I, Language>()
-> impl ChumskyParser<'tokens, 'src, I, FunctionType<'tokens, 'src, Language>>
where
    'src: 'tokens,
    I: super::traits::TokenInput<'tokens, 'src>,
    Language: LanguageChumskyParser<'tokens, 'src>,
{
    let input_types = Language::TypeLattice::parser()
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .or(empty().to(Vec::new()))
        .labelled("function input types");

    // output type can be -> type, or -> (type, type, ...)
    let output_type = just(Token::Arrow)
        .ignore_then(Language::TypeLattice::parser())
        .labelled("function output types");

    input_types
        .then(output_type)
        .map_with(|(param_types, return_type), extra| FunctionType {
            param_types,
            return_type,
            span: extra.span(),
        })
}
