use kirin_interpreter::{BranchCondition, Continuation, Interpretable, Interpreter};

use crate::ControlFlow;

impl<I, T> Interpretable<I> for ControlFlow<T>
where
    I: Interpreter,
    I::Value: Clone + BranchCondition,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            ControlFlow::Branch { target } => Ok(Continuation::Jump((*target).into(), vec![])),
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
                    Some(true) => Ok(Continuation::Jump((*true_target).into(), vec![])),
                    Some(false) => Ok(Continuation::Jump((*false_target).into(), vec![])),
                    None => Ok(Continuation::Fork(vec![
                        ((*true_target).into(), vec![]),
                        ((*false_target).into(), vec![]),
                    ])),
                }
            }
        }
    }
}
