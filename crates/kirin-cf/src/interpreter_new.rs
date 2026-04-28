use kirin::prelude::CompileTimeValue;
use kirin_interpreter_new::{
    BranchCondition, ConcreteTransfer, Env, Interpretable, InterpreterError, Location,
    StatementEffect,
};

use crate::ControlFlow;

impl<I, F, C, E, V, T> Interpretable<I, F, C, E, ConcreteTransfer<V>> for ControlFlow<T>
where
    I: Env<V, Error = E>,
    V: BranchCondition + Clone,
    E: From<IndeterminateBranch>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        match self {
            ControlFlow::Branch { target, args } => {
                let arguments = interp.read_many(env, args.as_slice())?;
                Ok(StatementEffect::Transfer(ConcreteTransfer::Jump {
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
                    None => return Err(IndeterminateBranch.into()),
                };
                let arguments = interp.read_many(env, args)?;
                Ok(StatementEffect::Transfer(ConcreteTransfer::Jump {
                    target,
                    arguments,
                }))
            }
            ControlFlow::__Phantom(..) => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndeterminateBranch;

impl std::fmt::Display for IndeterminateBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "indeterminate branch condition")
    }
}

impl std::error::Error for IndeterminateBranch {}

impl From<IndeterminateBranch> for InterpreterError {
    fn from(_: IndeterminateBranch) -> Self {
        Self::Custom("indeterminate branch condition")
    }
}
