use kirin::ir::*;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Dialect)]
#[kirin(terminator, fn, type_lattice = T)]
pub enum ControlFlow<T: TypeLattice> {
    Branch {
        target: Successor,
    },
    ConditionalBranch {
        condition: SSAValue,
        true_target: Successor,
        false_target: Successor,
        #[kirin(default = std::marker::PhantomData)]
        marker: std::marker::PhantomData<T>,
    },
    Return(SSAValue),
}
