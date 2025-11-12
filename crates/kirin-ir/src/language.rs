use crate::lattice::TypeLattice;

pub trait Language: Clone {
    type Type: TypeLattice;
}

pub trait HasArguments<'a> {
    type Iter: Iterator<Item = &'a crate::SSAValue>;
    fn arguments(&'a self) -> Self::Iter;
}

pub trait HasResults<'a> {
    type Iter: Iterator<Item = &'a crate::ResultValue>;
    fn results(&'a self) -> Self::Iter;
}

pub trait HasSuccessors<'a> {
    type Iter: Iterator<Item = &'a crate::Block>;
    fn successors(&'a self) -> Self::Iter;
}

pub trait HasRegions<'a> {
    type Iter: Iterator<Item = &'a crate::Region>;
    fn regions(&'a self) -> Self::Iter;
}

pub trait IsTerminator {
    fn is_terminator(&self) -> bool;
}

pub trait IsConstant {
    fn is_constant(&self) -> bool;
}

pub trait IsPure {
    fn is_pure(&self) -> bool;
}

/// An instruction combines several traits to provide a complete interface.
pub trait Statement<'a>: HasArguments<'a>
    + HasResults<'a>
    + HasSuccessors<'a>
    + HasRegions<'a>
    + IsTerminator
    + IsConstant
    + IsPure
{
}
