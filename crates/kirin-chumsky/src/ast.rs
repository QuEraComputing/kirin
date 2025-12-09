use super::traits::HasParser;
use kirin_ir::*;

#[derive(Debug, Clone)]
pub struct Block<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src>>
where
    'src: 'tokens,
    L::TypeLattice: HasParser<'tokens, 'src>,
{
    pub label: &'src str,
    pub arguments: Vec<(
        &'src str,
        <L::TypeLattice as HasParser<'tokens, 'src>>::Output,
    )>,
    pub statements: Vec<L::Output>,
}

#[derive(Debug, Clone)]
pub struct Region<'tokens, 'src: 'tokens, L: Dialect + HasParser<'tokens, 'src>>
where
    L::TypeLattice: HasParser<'tokens, 'src>,
{
    pub blocks: Vec<Block<'tokens, 'src, L>>,
}
