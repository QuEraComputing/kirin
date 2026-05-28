use kirin::prelude::{CompileTimeValue, Dialect};
use kirin_interpreter::{
    AbstractBlockTransfer, BranchCondition, ConcreteBlockTransfer, Env, Interpretable,
    InterpreterError, Location, StatementEffect,
};

use crate::ControlFlow;

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, ConcreteBlockTransfer<V>> for ControlFlow<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: BranchCondition + Clone,
    E: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteBlockTransfer<V>>, E> {
        match self {
            ControlFlow::Branch { target, args } => {
                let arguments = interp.read_many(env, args.as_slice())?;
                Ok(StatementEffect::Transfer(ConcreteBlockTransfer::Jump {
                    target: target.target(),
                    arguments,
                }))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                let (target, args) = match interp.read(env, *condition)?.is_truthy() {
                    Some(true) => (true_target.target(), true_args.as_slice()),
                    Some(false) => (false_target.target(), false_args.as_slice()),
                    None => return Err(E::from(InterpreterError::IndeterminateBranch)),
                };
                let arguments = interp.read_many(env, args)?;
                Ok(StatementEffect::Transfer(ConcreteBlockTransfer::Jump {
                    target,
                    arguments,
                }))
            }
            ControlFlow::__Phantom(..) => unreachable!(),
        }
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, AbstractBlockTransfer<V>> for ControlFlow<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: BranchCondition + Clone,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, AbstractBlockTransfer<V>>, E> {
        match self {
            ControlFlow::Branch { target, args } => {
                let arguments = interp.read_many(env, args.as_slice())?;
                Ok(StatementEffect::Transfer(AbstractBlockTransfer::Jump {
                    target: target.target(),
                    arguments,
                }))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                let (target, args) = match interp.read(env, *condition)?.is_truthy() {
                    Some(true) => (true_target.target(), true_args.as_slice()),
                    Some(false) => (false_target.target(), false_args.as_slice()),
                    None => {
                        let true_arguments = interp.read_many(env, true_args.as_slice())?;
                        let false_arguments = interp.read_many(env, false_args.as_slice())?;
                        return Ok(StatementEffect::Transfer(AbstractBlockTransfer::Branch {
                            true_target: true_target.target(),
                            true_arguments,
                            false_target: false_target.target(),
                            false_arguments,
                        }));
                    }
                };
                let arguments = interp.read_many(env, args)?;
                Ok(StatementEffect::Transfer(AbstractBlockTransfer::Jump {
                    target,
                    arguments,
                }))
            }
            ControlFlow::__Phantom(..) => unreachable!(),
        }
    }
}
