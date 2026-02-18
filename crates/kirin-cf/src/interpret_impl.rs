use kirin_interpreter::{InterpretControl, Interpretable, Interpreter};

use crate::ControlFlow;

impl<I, T> Interpretable<I> for ControlFlow<T>
where
    I: Interpreter,
    I::Value: Clone,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Control, I::Error> {
        match self {
            ControlFlow::Branch { target } => Ok(I::Control::ctrl_jump((*target).into(), vec![])),
            ControlFlow::Return(value) => {
                let v = interp.read(*value)?;
                Ok(I::Control::ctrl_return(v))
            }
            ControlFlow::ConditionalBranch { .. } => {
                // Domain-dependent: the wrapping dialect handles conditional branching
                // by inspecting the condition value and deciding which branch to take.
                Ok(I::Control::ctrl_continue())
            }
        }
    }
}
