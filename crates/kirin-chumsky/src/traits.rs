use std::fmt::Debug;

use crate::{operand, ssa_value};

use super::ast;
use chumsky::prelude::*;
use kirin_ir::*;
use kirin_lexer::Token;

pub trait TokenInput<'tokens, 'src: 'tokens>:
    chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = chumsky::span::SimpleSpan>
{
}

impl<'tokens, 'src: 'tokens, T> TokenInput<'tokens, 'src> for T where
    T: chumsky::input::ValueInput<'tokens, Token = Token<'src>, Span = chumsky::span::SimpleSpan>
{
}

pub type ParserError<'tokens, 'src> =
    extra::Err<Rich<'tokens, Token<'src>, chumsky::span::SimpleSpan>>;

pub trait HasParser<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>> {
    type Output: Clone + Debug;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>>;
}

impl<'tokens, 'src, L> HasParser<'tokens, 'src, L> for SSAValue
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L> + 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type Output = ast::Operand<'tokens, 'src, L>;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>> {
        operand().boxed()
    }
}

pub trait WithAbstractSyntaxTree<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>> {
    type AbstractSyntaxTreeNode;
}

impl<'tokens, 'src, L, T> WithAbstractSyntaxTree<'tokens, 'src, L> for std::marker::PhantomData<T>
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = std::marker::PhantomData<T>;
}

impl<'tokens, 'src, L, T> WithAbstractSyntaxTree<'tokens, 'src, L> for Vec<T>
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
    T: WithAbstractSyntaxTree<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = Vec<T::AbstractSyntaxTreeNode>;
}

impl<'tokens, 'src, L, T> WithAbstractSyntaxTree<'tokens, 'src, L> for Option<T>
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
    T: WithAbstractSyntaxTree<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = Option<T::AbstractSyntaxTreeNode>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for SSAValue
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = ast::Operand<'tokens, 'src, L>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for ResultValue
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = ast::ResultValue<'tokens, 'src, L>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for Block
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = ast::Block<'tokens, 'src, L>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for Successor
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = ast::BlockLabel<'src>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for Region
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = ast::Region<'tokens, 'src, L>;
}

macro_rules! impl_with_abstract_syntax_tree {
    ($name:ident) => {
        impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for $name
        where
            'src: 'tokens,
            L: Dialect + HasParser<'tokens, 'src, L>,
            L::TypeLattice: HasParser<'tokens, 'src, L>,
        {
            type AbstractSyntaxTreeNode = $name;
        }
    };
}

impl_with_abstract_syntax_tree!(u8);
impl_with_abstract_syntax_tree!(u16);
impl_with_abstract_syntax_tree!(u32);
impl_with_abstract_syntax_tree!(u64);
impl_with_abstract_syntax_tree!(i8);
impl_with_abstract_syntax_tree!(i16);
impl_with_abstract_syntax_tree!(i32);
impl_with_abstract_syntax_tree!(i64);
impl_with_abstract_syntax_tree!(f32);
impl_with_abstract_syntax_tree!(f64);
impl_with_abstract_syntax_tree!(bool);
impl_with_abstract_syntax_tree!(String);
