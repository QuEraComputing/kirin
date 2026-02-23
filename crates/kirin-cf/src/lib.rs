use kirin::prelude::*;

#[derive(Clone, Hash, PartialEq, Eq, Debug, Dialect, HasParser, PrettyPrint)]
#[kirin(terminator, fn, type = T)]
pub enum ControlFlow<T: CompileTimeValue + Default> {
    #[chumsky(format = "br {target}({args})")]
    Branch {
        target: Successor,
        args: Vec<SSAValue>,
    },
    #[chumsky(format = "cond_br {condition} then={true_target}({true_args}) else={false_target}({false_args})")]
    ConditionalBranch {
        condition: SSAValue,
        true_target: Successor,
        true_args: Vec<SSAValue>,
        false_target: Successor,
        false_args: Vec<SSAValue>,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[kirin(format = "ret {0}")]
    Return(SSAValue),
}

#[cfg(feature = "interpret")]
mod interpret_impl;
