use kirin::prelude::Dialect;
use kirin_interpreter::smallvec::smallvec;
use kirin_interpreter::{BranchCondition, Continuation, Interpretable, Interpreter};

use crate::ControlFlow;

impl<I, L, T> Interpretable<I, L> for ControlFlow<T>
where
    I: Interpreter,
    I::Value: Clone + BranchCondition,
    L: Dialect,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            ControlFlow::Branch { target } => {
                Ok(Continuation::Jump((*target).into(), smallvec![]))
            }
            ControlFlow::Return(value) => {
                let v = interp.read(*value)?;
                Ok(Continuation::Return(v))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                false_target,
                ..
            } => {
                let cond = interp.read(*condition)?;
                match cond.is_truthy() {
                    Some(true) => Ok(Continuation::Jump((*true_target).into(), smallvec![])),
                    Some(false) => {
                        Ok(Continuation::Jump((*false_target).into(), smallvec![]))
                    }
                    None => Ok(Continuation::Fork(smallvec![
                        ((*true_target).into(), smallvec![]),
                        ((*false_target).into(), smallvec![]),
                    ])),
                }
            }
        }
    }
}
