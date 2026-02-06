use crate::comptime::CompileTimeValue;

pub trait Lattice {
    fn join(&self, other: &Self) -> Self;
    fn meet(&self, other: &Self) -> Self;
    fn is_subseteq(&self, other: &Self) -> bool;
}

pub trait FiniteLattice: Lattice {
    fn bottom() -> Self;
    fn top() -> Self;
}

pub trait TypeLattice: FiniteLattice + CompileTimeValue + Default {}
