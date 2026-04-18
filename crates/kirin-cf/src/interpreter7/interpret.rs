use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_7::control::Control;
use kirin_interpreter_7::env::{Interp, Interpretable};

use crate::ControlFlow;

/// CF ops return `Control<E::Value, E::Ext>` — works for both concrete and
/// abstract modes with a single impl. The `Ext` type is only relevant if
/// `Control::Ext(...)` is returned; CF ops only use `Jump` and `Fork`.
impl<E, T> Interpretable<E> for ControlFlow<T>
where
    E: Interp,
    E::Value: Clone + BranchCondition,
    T: CompileTimeValue,
{
    type Effect = Control<E::Value, E::Ext>;

    fn interpret(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        match self {
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
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
