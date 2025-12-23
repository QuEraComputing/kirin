/// Some common AST structures for downstream dialect
/// to use with chumsky parsers.
use super::traits::HasParser;
use kirin_ir::*;

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: chumsky::span::SimpleSpan,
}

impl<T: Copy> Copy for Spanned<T> {}

impl<T: std::fmt::Display> std::fmt::Display for Spanned<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct FunctionType<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>>
where
    'src: 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    pub input_types: Vec<Spanned<<L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>>,
    pub output_types: Vec<Spanned<<L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>>,
}

#[derive(Debug, Clone)]
pub struct Operand<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>>
where
    'src: 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    pub name: Spanned<&'src str>,
    /// the type of the result value, if specified
    pub ty: Option<Spanned<<L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>>,
}

#[derive(Debug, Clone)]
pub struct ResultValue<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>>
where
    'src: 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    pub name: Spanned<&'src str>,
    /// the type of the result value, if specified
    pub ty: Option<Spanned<<L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>>,
}

#[derive(Debug, Clone)]
pub struct BlockLabel<'src> {
    pub name: Spanned<&'src str>,
}

#[derive(Debug, Clone)]
pub struct BlockArgument<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>>
where
    'src: 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    pub name: Spanned<&'src str>,
    pub ty: Spanned<<L::TypeLattice as HasParser<'tokens, 'src, L>>::Output>,
}

#[derive(Debug, Clone)]
pub struct BlockHeader<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>>
where
    'src: 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    pub label: BlockLabel<'src>,
    pub arguments: Vec<Spanned<BlockArgument<'tokens, 'src, L>>>,
}

#[derive(Debug, Clone)]
pub struct Block<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>>
where
    'src: 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    pub header: Spanned<BlockHeader<'tokens, 'src, L>>,
    pub statements: Vec<Spanned<L::Output>>,
}

#[derive(Debug, Clone)]
pub struct Region<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>>
where
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    pub blocks: Vec<Spanned<Block<'tokens, 'src, L>>>,
}
