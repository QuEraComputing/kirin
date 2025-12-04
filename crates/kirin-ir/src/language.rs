use crate::lattice::TypeLattice;

pub trait HasArguments<'a> {
    type Iter: Iterator<Item = &'a crate::SSAValue>;
    fn arguments(&'a self) -> Self::Iter;
}

pub trait HasArgumentsMut<'a> {
    type Iter: Iterator<Item = &'a mut crate::SSAValue>;
    fn arguments_mut(&'a mut self) -> Self::Iter;
}

pub trait HasResults<'a> {
    type Iter: Iterator<Item = &'a crate::ResultValue>;
    fn results(&'a self) -> Self::Iter;
}

pub trait HasResultsMut<'a> {
    type Iter: Iterator<Item = &'a mut crate::ResultValue>;
    fn results_mut(&'a mut self) -> Self::Iter;
}

pub trait HasSuccessors<'a> {
    type Iter: Iterator<Item = &'a crate::Block>;
    fn successors(&'a self) -> Self::Iter;
}

pub trait HasSuccessorsMut<'a> {
    type Iter: Iterator<Item = &'a mut crate::Block>;
    fn successors_mut(&'a mut self) -> Self::Iter;
}

pub trait HasRegions<'a> {
    type Iter: Iterator<Item = &'a crate::Region>;
    fn regions(&'a self) -> Self::Iter;
}

pub trait HasRegionsMut<'a> {
    type Iter: Iterator<Item = &'a mut crate::Region>;
    fn regions_mut(&'a mut self) -> Self::Iter;
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
pub trait Dialect:
    for<'a> HasArguments<'a>
    + for<'a> HasResults<'a>
    + for<'a> HasArgumentsMut<'a>
    + for<'a> HasResultsMut<'a>
    + for<'a> HasSuccessors<'a>
    + for<'a> HasSuccessorsMut<'a>
    + for<'a> HasRegions<'a>
    + for<'a> HasRegionsMut<'a>
    + IsTerminator
    + IsConstant
    + IsPure
    + Clone
    + PartialEq
    + std::fmt::Debug
{
    type TypeLattice: TypeLattice;
}
