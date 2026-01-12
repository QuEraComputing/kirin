use super::{
    ssa::{SSAValue, ssa_with_type},
    traits::*,
};
use chumsky::prelude::*;
use kirin_lexer::Token;

/// the name of a block, e.g
/// ```ignore
/// ^block_name
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BlockName<'src> {
    pub name: &'src str,
    pub span: SimpleSpan,
}

/// the head of a block, containing its name and arguments, e.g
/// ```ignore
/// ^block_name(%arg1: type1, %arg2: type2)
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BlockHead<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>> {
    pub name: BlockName<'src>,
    pub arguments: Vec<SSAValue<'tokens, 'src, Language>>,
}

/// a block containing a head and a sequence of statements, e.g
/// ```ignore
/// ^block_name(%arg1: type1, %arg2: type2) {
///     statement1
///     statement2
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Block<'tokens, 'src: 'tokens, Language: LanguageChumskyParser<'tokens, 'src>> {
    pub head: BlockHead<'tokens, 'src, Language>,
    pub statements: Vec<Language::Output>,
}

pub fn block_name<'tokens, 'src: 'tokens, I>()
-> impl ChumskyParser<'tokens, 'src, I, BlockName<'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Block(name) = e => BlockName {
        name,
        span: e.span(),
    } }
    .labelled("block name")
}

pub fn block_head<'tokens, 'src: 'tokens, I, Language>()
-> impl ChumskyParser<'tokens, 'src, I, BlockHead<'tokens, 'src, Language>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    Language: LanguageChumskyParser<'tokens, 'src>,
{
    let args_parser = ssa_with_type()
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .or(empty().to(Vec::new()))
        .labelled("block arguments");

    block_name()
        .then(args_parser)
        .map_with(|(name, arguments), _| BlockHead { name, arguments })
        .labelled("block head")
}

pub fn block<'tokens, 'src: 'tokens, I, Language>(
    language: RecursiveParser<'tokens, 'src, I, Language::Output>,
) -> impl ChumskyParser<'tokens, 'src, I, Block<'tokens, 'src, Language>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    Language: LanguageChumskyParser<'tokens, 'src>,
{
    let statements_parser = language.clone().repeated().collect().labelled("block statements");

    block_head()
        .then_ignore(just(Token::LBrace))
        .then(statements_parser)
        .then_ignore(just(Token::RBrace))
        .map_with(|(head, statements), _| Block { head, statements })
        .labelled("block")
}
