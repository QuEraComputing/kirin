use kirin::prelude::CompileTimeValue;
use kirin_interpreter::dialect::{
    BranchCondition, Edge, ForwardEffect, ForwardEval, ForwardEvalInterp, Interpretable,
};

use crate::ControlFlow;

/// One impl serves concrete and abstract execution: when the condition is
/// decided in the value domain we emit a [`ForwardEffect::Jump`]; when it is not
/// ([`BranchCondition::is_truthy`] returns `None`) we emit both edges and the
/// engine's policy decides (error under concrete execution, explore-and-join
/// under abstract interpretation).
impl<I, T> Interpretable<I, ForwardEval> for ControlFlow<T>
where
    I: ForwardEvalInterp,
    I::Value: BranchCondition,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        match self {
            ControlFlow::Branch { target, args } => Ok(ForwardEffect::Jump(Edge::new(
                target.target(),
                interp.read_many(args.as_slice())?,
            ))),
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => match interp.read(*condition)?.is_truthy() {
                Some(true) => Ok(ForwardEffect::Jump(Edge::new(
                    true_target.target(),
                    interp.read_many(true_args.as_slice())?,
                ))),
                Some(false) => Ok(ForwardEffect::Jump(Edge::new(
                    false_target.target(),
                    interp.read_many(false_args.as_slice())?,
                ))),
                None => Ok(ForwardEffect::Branch(vec![
                    Edge::new(
                        true_target.target(),
                        interp.read_many(true_args.as_slice())?,
                    ),
                    Edge::new(
                        false_target.target(),
                        interp.read_many(false_args.as_slice())?,
                    ),
                ])),
            },
            ControlFlow::__Phantom(..) => unreachable!(),
        }
    }
}
