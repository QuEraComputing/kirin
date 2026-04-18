use std::convert::Infallible;

use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::{AbstractValue, BranchCondition};
use kirin_interpreter_7::abstract_interp::AbstractInterp;
use kirin_interpreter_7::concrete::ConcreteInterp;
use kirin_interpreter_7::control::{Control, ControlExt};
use kirin_interpreter_7::env::Interpretable;
use kirin_interpreter_7::error::InterpreterError;
use kirin_interpreter_7::store::Store;

use crate::ControlFlow;

fn interp_impl<S, T, Ext>(
    op: &ControlFlow<T>,
    env: &mut S,
) -> Result<Control<S::Value, Ext>, S::Error>
where
    S: Store,
    S::Value: Clone + BranchCondition,
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
                    Ok(Control::Fork(
                        true_target.target(),
                        true_values,
                        false_target.target(),
                        false_values,
                    ))
                }
            }
        }
        ControlFlow::__Phantom(..) => unreachable!(),
    }
}

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for ControlFlow<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + BranchCondition,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, ControlExt<C>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
        interp_impl(self, env)
    }
}

impl<'ir, S, L, V, T> Interpretable<AbstractInterp<'ir, S, L, V>> for ControlFlow<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + BranchCondition,
    T: CompileTimeValue,
{
    type Effect = Control<V, Infallible>;

    fn interpret(
        &self,
        env: &mut AbstractInterp<'ir, S, L, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        interp_impl(self, env)
    }
}
