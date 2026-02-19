use crate::comptime::CompileTimeValue;

pub trait Lattice {
    fn join(&self, other: &Self) -> Self;
    fn meet(&self, other: &Self) -> Self;
    fn is_subseteq(&self, other: &Self) -> bool;
}

pub trait HasBottom: Lattice {
    fn bottom() -> Self;
}

pub trait HasTop: Lattice {
    fn top() -> Self;
}

pub trait FiniteLattice: HasBottom + HasTop {}

impl<T: HasBottom + HasTop> FiniteLattice for T {}

pub trait TypeLattice: FiniteLattice + CompileTimeValue + Default {}
