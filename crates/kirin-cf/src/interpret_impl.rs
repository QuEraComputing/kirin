use kirin::prelude::{Dialect, SSAValue};
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
            ControlFlow::Branch { target, args } => {
                let values = args
                    .iter()
                    .map(|v| interp.read(*v))
                    .collect::<Result<_, _>>()?;
                Ok(Continuation::Jump((*target).into(), values))
            }
            ControlFlow::Return(value) => {
                let v = interp.read(*value)?;
                Ok(Continuation::Return(v))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
                ..
            } => {
                let cond = interp.read(*condition)?;
                let read_args =
                    |interp: &mut I, args: &[SSAValue]| -> Result<_, I::Error> {
                        args.iter()
                            .map(|v| interp.read(*v))
                            .collect::<Result<_, _>>()
                    };
                match cond.is_truthy() {
                    Some(true) => {
                        let values = read_args(interp, true_args)?;
                        Ok(Continuation::Jump((*true_target).into(), values))
                    }
                    Some(false) => {
                        let values = read_args(interp, false_args)?;
                        Ok(Continuation::Jump((*false_target).into(), values))
                    }
                    None => {
                        let t_values = read_args(interp, true_args)?;
                        let f_values = read_args(interp, false_args)?;
                        Ok(Continuation::Fork(smallvec![
                            ((*true_target).into(), t_values),
                            ((*false_target).into(), f_values),
                        ]))
                    }
                }
            }
        }
    }
}
