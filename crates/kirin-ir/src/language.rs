use crate::comptime::CompileTimeValue;

// TODO: use Cow<'a, str> for name to avoid allocations in some cases

pub trait HasArguments<'a> {
    type Iter: Iterator<Item = &'a crate::SSAValue>;
    fn arguments(&'a self) -> Self::Iter;
}

pub trait HasArgumentsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::SSAValue>;
    fn arguments_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasResults<'a> {
    type Iter: Iterator<Item = &'a crate::ResultValue>;
    fn results(&'a self) -> Self::Iter;
}

pub trait HasResultsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::ResultValue>;
    fn results_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasBlocks<'a> {
    type Iter: Iterator<Item = &'a crate::Block>;
    fn blocks(&'a self) -> Self::Iter;
}

pub trait HasBlocksMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::Block>;
    fn blocks_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasSuccessors<'a> {
    type Iter: Iterator<Item = &'a crate::Successor>;
    fn successors(&'a self) -> Self::Iter;
}

pub trait HasSuccessorsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::Successor>;
    fn successors_mut(&'a mut self) -> Self::IterMut;
}

pub trait HasRegions<'a> {
    type Iter: Iterator<Item = &'a crate::Region>;
    fn regions(&'a self) -> Self::Iter;
}

pub trait HasRegionsMut<'a> {
    type IterMut: Iterator<Item = &'a mut crate::Region>;
    fn regions_mut(&'a mut self) -> Self::IterMut;
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

pub trait IsSpeculatable {
    fn is_speculatable(&self) -> bool;
}

/// An instruction combines several traits to provide a complete interface.
pub trait Dialect:
    for<'a> HasArguments<'a>
    + for<'a> HasResults<'a>
    + for<'a> HasArgumentsMut<'a>
    + for<'a> HasResultsMut<'a>
    + for<'a> HasBlocks<'a>
    + for<'a> HasBlocksMut<'a>
    + for<'a> HasSuccessors<'a>
    + for<'a> HasSuccessorsMut<'a>
    + for<'a> HasRegions<'a>
    + for<'a> HasRegionsMut<'a>
    + IsTerminator
    + IsConstant
    + IsPure
    + IsSpeculatable
    + Clone
    + PartialEq
    + std::fmt::Debug
{
    type Type: CompileTimeValue + Default;
}
