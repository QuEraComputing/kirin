use super::ast;
use super::lexer::Token;
use super::traits::{HasParser, ParserError, TokenInput};
use chumsky::prelude::*;
use kirin_ir::*;

// Box<dyn chumsky::Parser<'tokens, I, ast::Block<'tokens, 'src, L>, ParserError<'tokens, 'src>> + 'tokens>
pub fn block_parser<'tokens, 'src: 'tokens, I, L>(
    dialect: impl Parser<'tokens, I, L::Output, ParserError<'tokens, 'src>> + 'tokens,
) -> Boxed<'tokens, 'tokens, I, ast::Block<'tokens, 'src, L>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect + HasParser<'tokens, 'src> + 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src>,
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
        .labelled("block header");
    let statements = dialect
        .then_ignore(just(Token::Semicolon))
        .repeated()
        .collect::<Vec<_>>()
        .or(empty().to(Vec::new()))
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .labelled("block statements");

    header
        .then(statements)
        .map(|((label, arguments), statements)| ast::Block {
            label,
            arguments,
            statements,
        })
        .boxed()
}

pub fn region_parser<'tokens, 'src: 'tokens, I, L>(
    dialect: impl Parser<'tokens, I, L::Output, ParserError<'tokens, 'src>> + 'tokens,
) -> Boxed<'tokens, 'tokens, I, ast::Region<'tokens, 'src, L>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect + HasParser<'tokens, 'src> + 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src>,
{
    block_parser(dialect)
        .repeated()
        .collect::<Vec<_>>()
        .labelled("region blocks")
        .map(|blocks| ast::Region { blocks })
        .boxed()
}
