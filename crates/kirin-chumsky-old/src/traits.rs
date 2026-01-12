use std::{fmt::Debug, marker::PhantomData};

use crate::operand;

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

pub trait HasParser<'tokens, 'src: 'tokens, L: Dialect> {
    type Output: Clone + Debug + PartialEq;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>>;
}

impl<'tokens, 'src, L> HasParser<'tokens, 'src, L> for SSAValue
where
    'src: 'tokens,
    L: Dialect + 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type Output = ast::Operand<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>> {
        operand::<I, L>().boxed()
    }
}

impl<'tokens, 'src, T, L> HasParser<'tokens, 'src, L> for PhantomData<T>
where
    'src: 'tokens,
    T: 'tokens,
    L: Dialect,
{
    type Output = PhantomData<T>;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>> {
        empty().to(std::marker::PhantomData::<T>).boxed()
    }
}

// AST types implementing HasParser

impl<'tokens, 'src, L> HasParser<'tokens, 'src, L> for ast::Operand<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>
where
    'src: 'tokens,
    L: Dialect + 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type Output = ast::Operand<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>> {
        <SSAValue as HasParser<'tokens, 'src, L>>::parser::<I>()
    }
}

impl<'tokens, 'src, L> HasParser<'tokens, 'src, L> for ast::ResultValue<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>
where
    'src: 'tokens,
    L: Dialect + 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type Output = ast::ResultValue<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>> {
        operand::<I, L>().map(|op| ast::ResultValue {
            name: op.name,
            ty: op.ty,
        }).boxed()
    }
}

// Implement HasParser for kirin_ir types (ResultValue, etc)

impl<'tokens, 'src, L> HasParser<'tokens, 'src, L> for ResultValue
where
    'src: 'tokens,
    L: Dialect + 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type Output = ast::ResultValue<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>;
    fn parser<I: TokenInput<'tokens, 'src>>()
    -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>> {
        <ast::ResultValue<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output> as HasParser<'tokens, 'src, L>>::parser::<I>()
    }
}

// WithAbstractSyntaxTree needs update
pub trait WithAbstractSyntaxTree<'tokens, 'src: 'tokens, L: Dialect> {
    type AbstractSyntaxTreeNode: Debug + Clone;
}

// ... impls ...
impl<'tokens, 'src, L, T> WithAbstractSyntaxTree<'tokens, 'src, L> for std::marker::PhantomData<T>
where
    'src: 'tokens,
    L: Dialect,
{
    type AbstractSyntaxTreeNode = std::marker::PhantomData<T>;
}

impl<'tokens, 'src, L, T> WithAbstractSyntaxTree<'tokens, 'src, L> for Vec<T>
where
    'src: 'tokens,
    L: Dialect,
    T: WithAbstractSyntaxTree<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = Vec<T::AbstractSyntaxTreeNode>;
}

impl<'tokens, 'src, L, T> WithAbstractSyntaxTree<'tokens, 'src, L> for Option<T>
where
    'src: 'tokens,
    L: Dialect,
    T: WithAbstractSyntaxTree<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = Option<T::AbstractSyntaxTreeNode>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for SSAValue
where
    'src: 'tokens,
    L: Dialect,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = ast::Operand<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for ResultValue
where
    'src: 'tokens,
    L: Dialect,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = ast::ResultValue<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for Block
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = ast::Block<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output, L::Output>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for Successor
where
    'src: 'tokens,
    L: Dialect,
{
    type AbstractSyntaxTreeNode = ast::BlockLabel<'src>;
}

impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for Region
where
    'src: 'tokens,
    L: Dialect + HasParser<'tokens, 'src, L>,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    type AbstractSyntaxTreeNode = ast::Region<'src, <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output, L::Output>;
}

macro_rules! impl_with_abstract_syntax_tree {
    ($name:ident) => {
        impl<'tokens, 'src, L> WithAbstractSyntaxTree<'tokens, 'src, L> for $name
        where
            'src: 'tokens,
            L: Dialect,
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
