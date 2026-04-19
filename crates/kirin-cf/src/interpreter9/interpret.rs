use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_9::control::Control;
use kirin_interpreter_9::env::Env;
use kirin_interpreter_9::error::InterpreterError;
use kirin_interpreter_9::interpretable::Interpretable;

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
                Some(true) => {
                    let values = env.read_many(true_args)?;
                    Ok(Control::Jump(true_target.target(), values))
                }
                Some(false) => {
                    let values = env.read_many(false_args)?;
                    Ok(Control::Jump(false_target.target(), values))
                }
                None => {
                    let true_values = env.read_many(true_args)?;
                    let false_values = env.read_many(false_args)?;
                    Ok(Control::Fork(vec![
                        (true_target.target(), true_values),
                        (false_target.target(), false_values),
                    ]))
                }
            }
        }
        ControlFlow::__Phantom(..) => unreachable!(),
    }
}

/// Single generic impl for both concrete and abstract modes.
///
/// In concrete mode, `Fork` is unreachable (concrete BlockCursor handles Jump
/// and the concrete driver errors on Fork). In abstract mode, `Fork` causes
/// `AbstractBlockCursor` to call `enqueue_block` for each branch.
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
