use super::traits::HasParser;
use kirin_ir::*;

#[derive(Debug, Clone)]
pub struct Block<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>>
where
    'src: 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    pub label: &'src str,
    pub arguments: Vec<(
        &'src str,
        <L::TypeLattice as HasParser<'tokens, 'src, L>>::Output,
    )>,
    pub statements: Vec<L::Output>,
}

#[derive(Debug, Clone)]
pub struct Region<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src, L>>
where
    L::TypeLattice: HasParser<'tokens, 'src, L>,
{
    pub blocks: Vec<Block<'tokens, 'src, L>>,
}
