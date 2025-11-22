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
pub trait Statement<'a>: HasArguments<'a>
    + HasResults<'a>
    + HasArgumentsMut<'a>
    + HasResultsMut<'a>
    + HasSuccessors<'a>
    + HasSuccessorsMut<'a>
    + HasRegions<'a>
    + HasRegionsMut<'a>
    + IsTerminator
    + IsConstant
    + IsPure
{
}

pub trait Language: std::fmt::Debug + Clone + for<'a> Statement<'a> {
    type Type: TypeLattice;
}
