use super::ast;
use super::traits::{HasParser, TokenInput};
use super::lexer::Token;
use chumsky::prelude::*;
use kirin_ir::*;


impl<'tokens, 'src, L, E> HasParser<'tokens, 'src> for Block
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L, E>,
    L::TypeLattice: HasParser<'tokens, 'src, L, E>,
{
    type Output = ast::Block<'tokens, 'src, L>;

    fn parser<I>() -> impl chumsky::Parser<'tokens, Token<'src>, Self::Output, E>
    where
        I: TokenInput<'tokens, 'src>,
    {
        let label = just(Token::Caret)
            .ignore_then(select! { Token::Identifier(name) => name })
            .labelled("block label");
        let block_arg = just(Token::Percent)
            .ignore_then(select! { Token::Identifier(name) => name })
            .then_ignore(just(Token::Colon))
            .then(L::TypeLattice::parser::<I>())
            .labelled("block argument");
        let args_list = block_arg
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .or(empty().to(Vec::new()))
            .labelled("block arguments");
        let header = label
            .then(args_list)
            .then_ignore(just(Token::Colon))
            .labelled("block header");
        let statements = L::parser::<I>()
            .separated_by(just(Token::Semicolon))
            .collect::<Vec<_>>()
            .labelled("block statements");
        header
            .then(statements)
            .map(|((label, arguments), statements)| ast::Block {
                label,
                arguments,
                statements,
            })
    }
}

impl<'tokens, 'src, L, E> HasParser<'tokens, 'src, L, E>
    for Region
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L, E>,
    L::TypeLattice: HasParser<'tokens, 'src, L, E>,
{
    type Output = ast::Region<'tokens, 'src, L>;

    fn parser<I>() -> impl IRParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        let blocks = Block::parser::<I>()
            .separated_by(just(Token::Semicolon))
            .collect::<Vec<_>>()
            .labelled("region blocks");
        blocks.map(|blocks| ast::Region { blocks })
    }
}
