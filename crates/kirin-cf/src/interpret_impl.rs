use kirin::prelude::{Dialect, HasStageInfo, SSAValue};
use kirin_interpreter::{
    BranchCondition, Continuation, Interpretable, Interpreter, InterpreterError,
};
use smallvec::smallvec;

use crate::ControlFlow;

impl<'ir, I, T> Interpretable<'ir, I> for ControlFlow<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + BranchCondition,
    T: kirin::prelude::CompileTimeValue,
{
    fn interpret<L: Dialect>(
        &self,
        interp: &mut I,
    ) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            ControlFlow::Branch { target, args } => {
                let values = args
                    .iter()
                    .map(|v| interp.read(*v))
                    .collect::<Result<_, _>>()?;
                Ok(Continuation::Jump(target.target(), values))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                let cond = interp.read(*condition)?;
                let read_args = |interp: &mut I, args: &[SSAValue]| -> Result<_, I::Error> {
                    args.iter()
                        .map(|v| interp.read(*v))
                        .collect::<Result<_, _>>()
                };
                match cond.is_truthy() {
                    Some(true) => {
                        let values = read_args(interp, true_args)?;
                        Ok(Continuation::Jump(true_target.target(), values))
                    }
                    Some(false) => {
                        let values = read_args(interp, false_args)?;
                        Ok(Continuation::Jump(false_target.target(), values))
                    }
                    None => {
                        let t_values = read_args(interp, true_args)?;
                        let f_values = read_args(interp, false_args)?;
                        Ok(Continuation::Fork(smallvec![
                            (true_target.target(), t_values),
                            (false_target.target(), f_values),
                        ]))
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}
