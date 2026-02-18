use kirin::prelude::*;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Dialect, HasParser, PrettyPrint)]
#[kirin(terminator, fn, type = T)]
pub enum ControlFlow<T: CompileTimeValue + Default> {
    #[chumsky(format = "br {target}")]
    Branch { target: Successor },
    #[chumsky(format = "cond_br {condition} then={true_target} else={false_target}")]
    ConditionalBranch {
        condition: SSAValue,
        true_target: Successor,
        false_target: Successor,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[kirin(format = "ret {0}")]
    Return(SSAValue),
}

#[cfg(feature = "interpret")]
mod interpret_impl;
