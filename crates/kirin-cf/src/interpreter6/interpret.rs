use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_6::abstract_domain::BaseDomain;
use kirin_interpreter_6::core::Core;
use kirin_interpreter_6::env::Interpretable;
use kirin_interpreter_6::error::InterpreterError;
use kirin_interpreter_6::lift::{Lift, Project};

use crate::ControlFlow;

impl<E, T> Interpretable<E> for ControlFlow<T>
where
    E: BaseDomain,
    E::Value: Clone + BranchCondition,
    // Restated from BaseDomain's where clause — Rust does not automatically
    // propagate trait where-clause bounds to generic users of the trait.
    E::Effect: Lift<Core<E::Value, E::Cursor>> + Project<Core<E::Value, E::Cursor>>,
    T: CompileTimeValue,
    E::Error: From<InterpreterError>,
{
    type DialectEffect = E::Effect;

    fn interpret(&self, env: &mut E) -> Result<E::Effect, E::Error> {
        match self {
            ControlFlow::Branch { target, args } => {
                let values = env.read_many(args)?;
                Ok(E::Effect::lift(Core::Jump(target.target(), values)))
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
                        Ok(E::Effect::lift(Core::Jump(true_target.target(), values)))
                    }
                    Some(false) => {
                        let values = env.read_many(false_args)?;
                        Ok(E::Effect::lift(Core::Jump(false_target.target(), values)))
                    }
                    None => {
                        let true_values = env.read_many(true_args)?;
                        let false_values = env.read_many(false_args)?;
                        Ok(E::Effect::lift(Core::Fork(
                            true_target.target(),
                            true_values,
                            false_target.target(),
                            false_values,
                        )))
                    }
                }
            }
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
