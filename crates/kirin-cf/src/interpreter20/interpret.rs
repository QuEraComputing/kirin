use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_20::control::Control;
use kirin_interpreter_20::env::Env;
use kirin_interpreter_20::error::InterpreterError;
use kirin_interpreter_20::interpretable::Interpretable;

use crate::ControlFlow;

fn eval_impl<E, T>(op: &ControlFlow<T>, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error>
where
    E: Env,
    E::Value: BranchCondition + Clone,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    match op {
        ControlFlow::Branch { target, args } => {
            let values = env.read_many(args)?;
            Ok(Control::Jump(target.target(), values))
        }
        ControlFlow::ConditionalBranch {
            condition,
            true_target,
            true_args,
            false_target,
            false_args,
        } => {
            let cond = env.read(*condition)?;
            match cond.is_truthy() {
                Some(true) => Ok(Control::Jump(
                    true_target.target(),
                    env.read_many(true_args)?,
                )),
                Some(false) => Ok(Control::Jump(
                    false_target.target(),
                    env.read_many(false_args)?,
                )),
                None => Ok(Control::Fork(vec![
                    (true_target.target(), env.read_many(true_args)?),
                    (false_target.target(), env.read_many(false_args)?),
                ])),
            }
        }
        ControlFlow::__Phantom(..) => unreachable!(),
    }
}

impl<E, T> Interpretable<E> for ControlFlow<T>
where
    E: Env,
    E::Value: BranchCondition + Clone,
    E::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        eval_impl(self, env)
    }
}
