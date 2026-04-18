use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_6::concrete::ConcreteDomain;
use kirin_interpreter_6::core::Core;
use kirin_interpreter_6::env::Interpretable;
use kirin_interpreter_6::error::InterpreterError;
use kirin_interpreter_6::lift::{Lift, Project};

use crate::ControlFlow;

impl<E, V, T> Interpretable<E> for ControlFlow<T>
where
    E: ConcreteDomain<Value = V>,
    E::Effect: Lift<Core<V, E::Cursor>> + Project<Core<V, E::Cursor>>,
    V: Clone + BranchCondition,
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
                let (block, arg_slice) = match cond.is_truthy() {
                    Some(true) => (true_target.target(), true_args.as_slice()),
                    Some(false) => (false_target.target(), false_args.as_slice()),
                    None => {
                        return Err(E::Error::from(InterpreterError::UnhandledEffect(
                            "nondeterministic branch conditions are not supported in interpreter6"
                                .into(),
                        )));
                    }
                };
                let values = env.read_many(arg_slice)?;
                Ok(E::Effect::lift(Core::Jump(block, values)))
            }
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
