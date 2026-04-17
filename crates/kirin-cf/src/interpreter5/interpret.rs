use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_5::concrete::ConcreteDomain;
use kirin_interpreter_5::cursor::Boxed;
use kirin_interpreter_5::effect::ControlFlow as Cf5;
use kirin_interpreter_5::env::{Env, Interpretable};
use kirin_interpreter_5::error::InterpreterError;

use crate::ControlFlow;

impl<D, V, T> Interpretable<D> for ControlFlow<T>
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = Cf5<V, Boxed<D>>>,
    V: Clone + BranchCondition,
    T: CompileTimeValue,
{
    fn interpret(&self, domain: &mut D) -> Result<D::Effect, D::Error> {
        match self {
            ControlFlow::Branch { target, args } => {
                let values = domain.read_many(args)?;
                Ok(Cf5::Jump(target.target(), values))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                let cond = domain.read(*condition)?;
                let (block, arg_slice) = match cond.is_truthy() {
                    Some(true) => (true_target.target(), true_args.as_slice()),
                    Some(false) => (false_target.target(), false_args.as_slice()),
                    None => {
                        return Err(D::Error::from(InterpreterError::UnhandledEffect(
                            "nondeterministic branch conditions are not supported in interpreter5"
                                .into(),
                        )));
                    }
                };
                let values = domain.read_many(arg_slice)?;
                Ok(Cf5::Jump(block, values))
            }
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
