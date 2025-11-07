use crate::lattice::TypeLattice;

pub trait Language: Clone {
    type Type: TypeLattice;
}

/// minimal information about an instruction.
pub trait Instruction {
    fn arguments(&self) -> impl Iterator<Item = crate::node::SSAValue>;
    fn results(&self) -> impl Iterator<Item = crate::node::ResultValue>;
    fn is_terminator(&self) -> bool;
    fn successors(&self) -> impl Iterator<Item = crate::node::Block>;
}
