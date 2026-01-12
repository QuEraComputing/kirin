use kirin::ir::*;
// use kirin::parsers::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[wraps]
#[kirin(fn, type_lattice = T)]
pub enum StructuredControlFlow<T: TypeLattice> {
    If(If<T>),
    For(For<T>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(fn, type_lattice = T)]
pub struct If<T: TypeLattice> {
    condition: SSAValue,
    then_body: Block,
    else_body: Block,
    #[kirin(default = std::marker::PhantomData)]
    marker: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(fn, type_lattice = T)]
pub struct For<T: TypeLattice> {
    induction_var: SSAValue,
    start: SSAValue,
    end: SSAValue,
    step: SSAValue,
    body: Block,
    #[kirin(default = std::marker::PhantomData)]
    marker: std::marker::PhantomData<T>,
}
