use std::convert::Infallible;

use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::{AbstractValue, BranchCondition};
use kirin_interpreter_8::abstract_interp::AbstractInterp;
use kirin_interpreter_8::concrete::ConcreteInterp;
use kirin_interpreter_8::control::{Control, CursorExt};
use kirin_interpreter_8::env::Env;
use kirin_interpreter_8::error::InterpreterError;
use kirin_interpreter_8::semantics::Semantics;

use crate::ControlFlow;

fn eval_impl<D, T, Ext>(
    op: &ControlFlow<T>,
    domain: &mut D,
) -> Result<Control<D::Value, Ext>, D::Error>
where
    D: Env,
    D::Value: Clone + BranchCondition,
    T: CompileTimeValue,
{
    match op {
        ControlFlow::Branch { target, args } => {
            let values = domain.read_many(args)?;
            Ok(Control::Jump(target.target(), values))
        }
        ControlFlow::ConditionalBranch {
            condition,
            true_target,
            true_args,
            false_target,
            false_args,
        } => {
            let cond = domain.read_value(*condition)?;
            match cond.is_truthy() {
                Some(true) => {
                    let values = domain.read_many(true_args)?;
                    Ok(Control::Jump(true_target.target(), values))
                }
                Some(false) => {
                    let values = domain.read_many(false_args)?;
                    Ok(Control::Jump(false_target.target(), values))
                }
                None => {
                    let true_values = domain.read_many(true_args)?;
                    let false_values = domain.read_many(false_args)?;
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

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for ControlFlow<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + BranchCondition,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, CursorExt<C>>;

    fn eval(
        &self,
        domain: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        eval_impl(self, domain)
    }
}

impl<'ir, S, L, V, T> Semantics<AbstractInterp<'ir, S, L, V>> for ControlFlow<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue + BranchCondition,
    T: CompileTimeValue,
{
    type Effect = Control<V, Infallible>;

    fn eval(
        &self,
        domain: &mut AbstractInterp<'ir, S, L, V>,
    ) -> Result<Control<V, Infallible>, InterpreterError> {
        eval_impl(self, domain)
    }
}
