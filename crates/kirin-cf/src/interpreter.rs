use kirin::prelude::CompileTimeValue;
use kirin_interpreter::dialect::{BranchCondition, Ctx, Edge, Effect, Interp, Interpretable};

use crate::ControlFlow;

/// One impl serves concrete and abstract execution: when the condition is
/// decided in the value domain we emit a [`Effect::Jump`]; when it is not
/// ([`BranchCondition::is_truthy`] returns `None`) we emit both edges and the
/// engine's policy decides (error under concrete execution, explore-and-join
/// under abstract interpretation).
impl<I, T> Interpretable<I> for ControlFlow<T>
where
    I: Interp,
    I::Value: BranchCondition,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error> {
        match self {
            ControlFlow::Branch { target, args } => Ok(Effect::Jump(Edge::new(
                target.target(),
                ctx.read_many(args.as_slice())?,
            ))),
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => match ctx.read(*condition)?.is_truthy() {
                Some(true) => Ok(Effect::Jump(Edge::new(
                    true_target.target(),
                    ctx.read_many(true_args.as_slice())?,
                ))),
                Some(false) => Ok(Effect::Jump(Edge::new(
                    false_target.target(),
                    ctx.read_many(false_args.as_slice())?,
                ))),
                None => Ok(Effect::Branch(vec![
                    Edge::new(true_target.target(), ctx.read_many(true_args.as_slice())?),
                    Edge::new(false_target.target(), ctx.read_many(false_args.as_slice())?),
                ])),
            },
            ControlFlow::__Phantom(..) => unreachable!(),
        }
    }
}
