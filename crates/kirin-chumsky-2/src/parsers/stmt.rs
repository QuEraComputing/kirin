use super::ssa::{ResultValue, result_value};
use super::traits::*;
use chumsky::prelude::*;
use kirin_lexer::Token;

/// a statement consisting of a right-hand side expression
/// and a list of left-hand side result values, e.g
/// ```ignore
/// %result1, %result2 = rhs_expression
/// ```
///
/// the right hand side expression can contain type specifications
/// for the result values by providing, e.g
///
/// ```ignore
/// "add {lhs} {rhs} -> {result:type}"
/// ```
pub struct Statement<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>> {
    pub rhs: Language::Output,
    pub lhs: Vec<ResultValue<'src>>,
}

pub fn statement<'tokens, 'src: 'tokens, I, Language>(
    language: RecursiveParser<'tokens, 'src, I, Language::Output>,
) -> impl ChumskyParser<'tokens, 'src, I, Statement<'tokens, 'src, Language>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    Language: LanguageChumskyParser<'tokens, 'src>,
{
    result_value()
        .separated_by(just(Token::Comma))
        .collect()
        .then_ignore(just(Token::Equal))
        .then(language)
        .map(|(lhs, rhs)| Statement { lhs, rhs })
        .labelled("statement")
}
