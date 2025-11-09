use crate::lattice::TypeLattice;

pub trait Language: Clone {
    type Type: TypeLattice;
}

pub trait HasArguments {
    fn arguments(&self) -> impl Iterator<Item = &crate::SSAValue>;
}

pub trait HasResults {
    fn results(&self) -> impl Iterator<Item = &crate::ResultValue>;
}

pub trait HasSuccessors {
    fn successors(&self) -> impl Iterator<Item = &crate::Block>;
}

pub trait HasRegions {
    fn regions(&self) -> impl Iterator<Item = &crate::Region>;
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
pub trait Instruction:
    HasArguments + HasResults + HasSuccessors + HasRegions + IsTerminator + IsConstant + IsPure
{
}
