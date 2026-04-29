use kirin::prelude::{CompileTimeValue, Dialect};
use kirin_interpreter_new::{
    BlockTransfer, BranchCondition, Env, Interpretable, InterpreterError, Location, StatementEffect,
};

use crate::ControlFlow;

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, BlockTransfer<V>> for ControlFlow<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: BranchCondition + Clone,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        match self {
            ControlFlow::Branch { target, args } => {
                let arguments = interp.read_many(env, args.as_slice())?;
                Ok(StatementEffect::Transfer(BlockTransfer::Jump {
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
                        return Ok(StatementEffect::Transfer(BlockTransfer::Branch {
                            true_target: true_target.target(),
                            true_arguments,
                            false_target: false_target.target(),
                            false_arguments,
                        }));
                    }
                };
                let arguments = interp.read_many(env, args)?;
                Ok(StatementEffect::Transfer(BlockTransfer::Jump {
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
