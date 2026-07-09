use kirin::prelude::{Block, CompileTimeValue, HasBottom, SSAValue};
use kirin_interpreter::dialect::{
    BranchCondition, ClassicLiveness, ClassicLivenessInterp, DemandInterp, DenseBackwardEffect,
    Edge, ForwardEval, Interpretable, SparseForwardEffect, SparseForwardInterp, StrongDemand,
    SuccessorEdge,
};

use crate::ControlFlow;

/// Classic (weak) per-point liveness: the branch condition is a use (gen'd);
/// edge arguments are mapped at the block boundary from the successors'
/// converged live-in sets, so the rule only *names* its edges.
impl<I, T> Interpretable<I, ClassicLiveness> for ControlFlow<T>
where
    I: ClassicLivenessInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        match self {
            ControlFlow::Branch { target, args } => {
                Ok(DenseBackwardEffect::Edges(vec![SuccessorEdge {
                    target: target.target(),
                    args: args.clone(),
                }]))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                interp.gen_live(*condition)?;
                Ok(DenseBackwardEffect::Edges(vec![
                    SuccessorEdge {
                        target: true_target.target(),
                        args: true_args.clone(),
                    },
                    SuccessorEdge {
                        target: false_target.target(),
                        args: false_args.clone(),
                    },
                ]))
            }
            ControlFlow::__Phantom(..) => unreachable!(),
        }
    }
}

/// Demand the edge arguments whose matching successor parameters are demanded.
fn demand_edge_args<I>(interp: &mut I, target: Block, args: &[SSAValue]) -> Result<(), I::Error>
where
    I: DemandInterp,
    I::Value: HasBottom + PartialEq,
{
    let params = interp.block_params(target)?;
    for (param, arg) in params.iter().zip(args) {
        if interp.is_demanded(*param)? {
            interp.demand(*arg)?;
        }
    }
    Ok(())
}

/// Backward demand: the branch condition is an unconditional control root; an
/// edge argument is demanded iff its matching successor block parameter is
/// demanded. The rule knows its own edge layout, so no positional operand
/// grouping is ever guessed.
impl<I, T> Interpretable<I, StrongDemand> for ControlFlow<T>
where
    I: DemandInterp,
    I::Value: HasBottom + PartialEq,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        match self {
            ControlFlow::Branch { target, args } => {
                demand_edge_args(interp, target.target(), args)?;
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                interp.demand(*condition)?;
                demand_edge_args(interp, true_target.target(), true_args)?;
                demand_edge_args(interp, false_target.target(), false_args)?;
            }
            ControlFlow::__Phantom(..) => unreachable!(),
        }
        Ok(interp.effect())
    }
}

/// One impl serves concrete and abstract execution: when the condition is
/// decided in the value domain we emit a [`SparseForwardEffect::Jump`]; when it is not
/// ([`BranchCondition::is_truthy`] returns `None`) we emit both edges and the
/// engine's policy decides (error under concrete execution, explore-and-join
/// under abstract interpretation).
impl<I, T> Interpretable<I, ForwardEval> for ControlFlow<T>
where
    I: SparseForwardInterp,
    I::Value: BranchCondition,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        match self {
            ControlFlow::Branch { target, args } => Ok(SparseForwardEffect::Jump(Edge::new(
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
                Some(true) => Ok(SparseForwardEffect::Jump(Edge::new(
                    true_target.target(),
                    interp.read_many(true_args.as_slice())?,
                ))),
                Some(false) => Ok(SparseForwardEffect::Jump(Edge::new(
                    false_target.target(),
                    interp.read_many(false_args.as_slice())?,
                ))),
                None => Ok(SparseForwardEffect::Branch(vec![
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
