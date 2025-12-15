use kirin::ir::*;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Dialect, HasParser)]
#[kirin(terminator, fn, type_lattice = T)]
pub enum ControlFlow<T: TypeLattice> {
    #[text("br {target}")]
    Branch { target: Successor },
    #[kirin(text = "cond_br {condition} then={true_target} else={false_target}")]
    ConditionalBranch {
        condition: SSAValue,
        true_target: Successor,
        false_target: Successor,
        #[kirin(default = std::marker::PhantomData)]
        marker: std::marker::PhantomData<T>,
    },
    #[text("ret {0}")]
    Return(SSAValue),
}
