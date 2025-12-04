use kirin::ir::*;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Dialect)]
#[kirin(terminator, fn, type_lattice = T)]
pub enum ControlFlow<T: TypeLattice> {
    Branch {
        target: Block,
    },
    ConditionalBranch {
        condition: SSAValue,
        true_target: Block,
        false_target: Block,
        #[kirin(default = std::marker::PhantomData)]
        marker: std::marker::PhantomData<T>,
    },
    Return(SSAValue),
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_cf() {
//         // ControlFlow::op_conditional_branch(context, condition, true_target, false_target);
//         let inst = ControlFlow::Return(TestSSAValue(0).into());
//         for succ in inst.successors() {
//             println!("Successor: {:?}", succ);
//         }
//     }
// }
