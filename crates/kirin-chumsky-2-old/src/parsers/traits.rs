use chumsky::prelude::*;
use chumsky::recursive::{Direct, Recursive};
use kirin_ir::Dialect;
use kirin_lexer::Token;

/// An alias for token input types used in Kirin Chumsky parsers
pub trait TokenInput<'tokens, 'src: 'tokens>:
    chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>
{
}

impl<'tokens, 'src: 'tokens, I> TokenInput<'tokens, 'src> for I where
    I: chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = SimpleSpan>
{
}

/// A alias trait for Chumsky parsers used in Kirin
pub trait ChumskyParser<'tokens, 'src: 'tokens, I, O>:
    Parser<'tokens, I, O, ParserError<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
{
}

impl<'tokens, 'src: 'tokens, I, O, P> ChumskyParser<'tokens, 'src, I, O> for P
where
    I: TokenInput<'tokens, 'src>,
    P: Parser<'tokens, I, O, ParserError<'tokens, 'src>>,
{
}

pub type RecursiveParser<'tokens, 'src, I, O> =
    Recursive<Direct<'tokens, 'tokens, I, O, ParserError<'tokens, 'src>>>;
pub type ParserError<'tokens, 'src> = extra::Err<Rich<'tokens, Token<'src>, SimpleSpan>>;
pub type BoxedParser<'tokens, 'src, I, O> =
    Boxed<'tokens, 'tokens, I, O, ParserError<'tokens, 'src>>;

/// trait for types that have an associated Chumsky parser
pub trait WithChumskyParser<'tokens, 'src: 'tokens> {
    type Output: Clone + std::fmt::Debug + PartialEq;
    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>;
}

pub trait WithRecursiveChumskyParser<
    'tokens,
    'src: 'tokens,
    Language: LanguageChumskyParser<'tokens, 'src>,
>
{
    type Output: Clone + std::fmt::Debug + PartialEq;
    fn recursive<I>(
        language: RecursiveParser<'tokens, 'src, I, Language::Output>,
    ) -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>;
}

pub trait LanguageChumskyParser<'tokens, 'src: 'tokens>:
    Dialect<TypeLattice: WithChumskyParser<'tokens, 'src>> + WithRecursiveChumskyParser<'tokens, 'src, Self>
{
}

impl<'tokens, 'src: 'tokens, L> LanguageChumskyParser<'tokens, 'src> for L where
    L: Dialect<TypeLattice: WithChumskyParser<'tokens, 'src>> + WithRecursiveChumskyParser<'tokens, 'src, Self>
{
}

impl<'tokens, 'src: 'tokens, Node> WithChumskyParser<'tokens, 'src> for Node
where
    Node: LanguageChumskyParser<'tokens, 'src> + 'tokens,
{
    type Output = <Node as WithRecursiveChumskyParser<'tokens, 'src, Self>>::Output;
    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        chumsky::recursive::recursive(|language| Node::recursive(language)).boxed()
    }
}
