use super::ast;
use super::traits::{HasParser, ParserError, TokenInput};
use chumsky::prelude::*;
use kirin_ir::*;
use kirin_lexer::Token;

pub fn identifier<'tokens, 'src: 'tokens, I>(
    name: &'src str,
) -> impl Parser<'tokens, I, ast::Spanned<&'src str>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Identifier(id) = e if id == name => ast::Spanned {
        value: id,
        span: e.span(),
    }}
    .labelled(format!("identifier '{}'", name))
}

pub fn symbol<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, ast::Spanned<&'src str>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Symbol(sym) = e => ast::Spanned {
        value: sym,
        span: e.span(),
    }}
    .labelled("symbol")
}

pub fn operand<'tokens, 'src: 'tokens, I, L>()
-> impl Parser<'tokens, I, ast::Operand<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    ssa_value()
        .then(
            just(Token::Colon)
                .ignore_then(L::TypeLattice::parser())
                .or_not(),
        )
        .map(|(name, ty)| ast::Operand { name, ty })
        .labelled("operand")
}

pub fn operands<'tokens, 'src: 'tokens, I, L>(
    n: usize,
    sep: Token<'src>,
) -> impl Parser<'tokens, I, Vec<ast::Operand<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    operand::<I, L>()
        .separated_by(just(sep))
        .exactly(n)
        .collect()
        .labelled(format!("{} operands", n))
}

pub fn result_values<'tokens, 'src: 'tokens, I, L>(
    n: usize,
) -> impl Parser<'tokens, I, Vec<ast::ResultValue<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    ssa_value()
        .map(|name| ast::ResultValue { name, ty: None })
        .separated_by(just(Token::Comma))
        .exactly(n)
        .collect()
        .labelled(format!("{} result values", n))
        .then_ignore(just(Token::Equal))
}

pub fn literal_int<'tokens, 'src: 'tokens, T, I>(
    f: impl Fn(&str, SimpleSpan) -> Result<T, Rich<'tokens, Token<'src>, chumsky::span::SimpleSpan>>
    + 'tokens,
) -> impl Parser<'tokens, I, ast::Spanned<T>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Int(v) = e => f(v, e.span()) }
        .try_map(|res, span| match res {
            Ok(v) => Ok(ast::Spanned { value: v, span }),
            Err(e) => Err(e),
        })
        .labelled("literal integer")
}

pub fn literal_float<'tokens, 'src: 'tokens, T, I>(
    f: impl Fn(&str, SimpleSpan) -> Result<T, Rich<'tokens, Token<'src>, chumsky::span::SimpleSpan>>
    + 'tokens,
) -> impl Parser<'tokens, I, ast::Spanned<T>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Float(v) = e => f(v, e.span()) }
        .try_map(|res, span| match res {
            Ok(v) => Ok(ast::Spanned { value: v, span }),
            Err(e) => Err(e),
        })
        .labelled("literal float")
}

pub fn function_type<'tokens, 'src: 'tokens, I, L>()
-> impl Parser<'tokens, I, ast::Spanned<ast::FunctionType<<L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    let input_types = L::TypeLattice::parser::<I>()
        .map_with(|ty, e| ast::Spanned {
            value: ty,
            span: e.span(),
        })
        .separated_by(just(Token::Comma))
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .or(empty().to(Vec::new()))
        .labelled("function input types");

    let output_types = just(Token::Arrow)
        .ignore_then(
            L::TypeLattice::parser::<I>()
                .map_with(|ty, e| ast::Spanned {
                    value: ty,
                    span: e.span(),
                })
                .separated_by(just(Token::Comma))
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .or(L::TypeLattice::parser::<I>().map_with(|ty, e| {
                    vec![ast::Spanned {
                        value: ty,
                        span: e.span(),
                    }]
                }))
                .or(empty().to(Vec::new())),
        )
        .labelled("function output types");

    input_types
        .then(output_types)
        .map_with(|(input_types, output_types), e| ast::Spanned {
            value: ast::FunctionType {
                input_types,
                output_types,
            },
            span: e.span(),
        })
}

pub fn block_label<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, ast::BlockLabel<'src>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Block(name) = e => ast::Spanned {
        value: name,
        span: e.span(),
    }}
    .map(|spanned| ast::BlockLabel {
        name: ast::Spanned {
            value: spanned.value,
            span: spanned.span,
        },
    })
    .labelled("block label")
}

pub fn ssa_value<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, ast::Spanned<&'src str>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
{
    select! {
        Token::SSAValue(name) = e => ast::Spanned {
            value: name,
            span: e.span(),
        }
    }
    .labelled("SSA Value")
}

pub fn block_argument<'tokens, 'src: 'tokens, I, L>() -> impl Parser<
    'tokens,
    I,
    ast::Spanned<ast::BlockArgument<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>>,
    ParserError<'tokens, 'src>,
>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    ssa_value()
        .then_ignore(just(Token::Colon))
        .then(
            L::TypeLattice::parser::<I>().map_with(|ty, e| ast::Spanned {
                value: ty,
                span: e.span(),
            }),
        )
        .labelled("block argument")
        .map_with(|(name, ty), e| ast::Spanned {
            value: ast::BlockArgument { name, ty },
            span: e.span(),
        })
}

pub fn block_argument_list<'tokens, 'src: 'tokens, I, L>() -> impl Parser<
    'tokens,
    I,
    Vec<ast::Spanned<ast::BlockArgument<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>>>,
    ParserError<'tokens, 'src>,
>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    block_argument::<I, L>()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
        .or(empty().to(Vec::new()))
        .labelled("block arguments")
}

pub fn block_header<'tokens, 'src: 'tokens, I, L>()
-> impl Parser<'tokens, I, ast::Spanned<ast::BlockHeader<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    block_label()
        .then(block_argument_list::<_, L>())
        .labelled("block header")
        .map_with(|(label, arguments), e| ast::Spanned {
            value: ast::BlockHeader { label, arguments },
            span: e.span(),
        })
}

pub fn block<'tokens, 'src: 'tokens, I, L>(
    dialect: impl Parser<'tokens, I, L::Output, ParserError<'tokens, 'src>> + 'tokens,
) -> impl Parser<'tokens, I, ast::Spanned<ast::Block<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output, L::Output>>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    let header = block_header::<_, L>();
    let statements = dialect
        .map_with(|stmt, e| ast::Spanned {
            value: stmt,
            span: e.span(),
        })
        .then_ignore(just(Token::Semicolon))
        .repeated()
        .collect::<Vec<_>>()
        .or(empty().to(Vec::new()))
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .labelled("block statements");

    header
        .then(statements)
        .map_with(|(header, statements), e| ast::Spanned {
            value: ast::Block { header, statements },
            span: e.span(),
        })
}

pub fn region<'tokens, 'src: 'tokens, I, L>(
    dialect: impl Parser<'tokens, I, L::Output, ParserError<'tokens, 'src>> + 'tokens,
) -> impl Parser<'tokens, I, ast::Region<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output, L::Output>, ParserError<'tokens, 'src>>
where
    'src: 'tokens,
    I: TokenInput<'tokens, 'src>,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    block::<I, L>(dialect)
        .then_ignore(just(Token::Semicolon).or_not())
        .repeated()
        .collect::<Vec<_>>()
        .labelled("region blocks")
        .map(|blocks| ast::Region { blocks })
}