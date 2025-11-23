use crate::TypeLattice;

pub trait CompileTimeValue: Clone + std::fmt::Debug + std::hash::Hash + PartialEq {}

pub trait Typeof<L: TypeLattice> {
    fn type_of(&self) -> L;
}
