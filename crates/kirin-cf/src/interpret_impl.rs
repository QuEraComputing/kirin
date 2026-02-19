use kirin_interpreter::{BranchCondition, InterpretControl, Interpretable, Interpreter};

use crate::ControlFlow;

impl<I, T> Interpretable<I> for ControlFlow<T>
where
    I: Interpreter,
    I::Value: Clone + BranchCondition,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Control, I::Error> {
        match self {
            ControlFlow::Branch { target } => Ok(I::Control::ctrl_jump((*target).into(), vec![])),
            ControlFlow::Return(value) => {
                let v = interp.read(*value)?;
                Ok(I::Control::ctrl_return(v))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                false_target,
                ..
            } => {
                let cond = interp.read(*condition)?;
                match cond.is_truthy() {
                    Some(true) => Ok(I::Control::ctrl_jump((*true_target).into(), vec![])),
                    Some(false) => Ok(I::Control::ctrl_jump((*false_target).into(), vec![])),
                    None => Ok(I::Control::ctrl_fork(vec![
                        ((*true_target).into(), vec![]),
                        ((*false_target).into(), vec![]),
                    ])),
                }
            }
        }
    }
}
