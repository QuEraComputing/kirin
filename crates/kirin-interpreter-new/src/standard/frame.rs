use core::convert::Infallible;

use kirin_ir::TryLiftFrom;

use crate::{ConcreteBlockTransfer, Frame, FrameEffect, HasLocation, Location};

use super::{
    AbstractBranchFrame, BlockFrame, CallFrame, FunctionFrame, RegionFrame,
    SpecializedFunctionFrame, StagedFunctionFrame, StatementFrame,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StandardFrame<L, V, T = ConcreteBlockTransfer<V>> {
    Statement(StatementFrame),
    AbstractBranch(AbstractBranchFrame<L, V>),
    Block(BlockFrame<L, V, T>),
    Region(RegionFrame<L, V, T>),
    Call(CallFrame<L, V>),
    Function(FunctionFrame<L, V>),
    StagedFunction(StagedFunctionFrame<L, V>),
    SpecializedFunction(SpecializedFunctionFrame<L, V>),
}

impl<L, V, T> TryLiftFrom<StatementFrame> for StandardFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: StatementFrame) -> Result<Self, Self::Error> {
        Ok(Self::Statement(frame))
    }
}

impl<L, V, T> TryLiftFrom<AbstractBranchFrame<L, V>> for StandardFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: AbstractBranchFrame<L, V>) -> Result<Self, Self::Error> {
        Ok(Self::AbstractBranch(frame))
    }
}

impl<L, V, T> TryLiftFrom<BlockFrame<L, V, T>> for StandardFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: BlockFrame<L, V, T>) -> Result<Self, Self::Error> {
        Ok(Self::Block(frame))
    }
}

impl<L, V, T> TryLiftFrom<RegionFrame<L, V, T>> for StandardFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: RegionFrame<L, V, T>) -> Result<Self, Self::Error> {
        Ok(Self::Region(frame))
    }
}

impl<L, V, T> TryLiftFrom<CallFrame<L, V>> for StandardFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: CallFrame<L, V>) -> Result<Self, Self::Error> {
        Ok(Self::Call(frame))
    }
}

impl<L, V, T> TryLiftFrom<FunctionFrame<L, V>> for StandardFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: FunctionFrame<L, V>) -> Result<Self, Self::Error> {
        Ok(Self::Function(frame))
    }
}

impl<L, V, T> TryLiftFrom<StagedFunctionFrame<L, V>> for StandardFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: StagedFunctionFrame<L, V>) -> Result<Self, Self::Error> {
        Ok(Self::StagedFunction(frame))
    }
}

impl<L, V, T> TryLiftFrom<SpecializedFunctionFrame<L, V>> for StandardFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: SpecializedFunctionFrame<L, V>) -> Result<Self, Self::Error> {
        Ok(Self::SpecializedFunction(frame))
    }
}

impl<L, V, T> HasLocation for StandardFrame<L, V, T> {
    fn location(&self) -> Location {
        match self {
            Self::Statement(frame) => frame.location(),
            Self::AbstractBranch(frame) => frame.location(),
            Self::Block(frame) => frame.location(),
            Self::Region(frame) => frame.location(),
            Self::Call(frame) => frame.location(),
            Self::Function(frame) => frame.location(),
            Self::StagedFunction(frame) => frame.location(),
            Self::SpecializedFunction(frame) => frame.location(),
        }
    }
}

impl<I, L, F, C, E, V, T> Frame<I, F, C, E> for StandardFrame<L, V, T>
where
    StatementFrame: Frame<I, F, C, E>,
    AbstractBranchFrame<L, V>: Frame<I, F, C, E>,
    BlockFrame<L, V, T>: Frame<I, F, C, E>,
    RegionFrame<L, V, T>: Frame<I, F, C, E>,
    CallFrame<L, V>: Frame<I, F, C, E>,
    FunctionFrame<L, V>: Frame<I, F, C, E>,
    StagedFunctionFrame<L, V>: Frame<I, F, C, E>,
    SpecializedFunctionFrame<L, V>: Frame<I, F, C, E>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self {
            Self::Statement(frame) => frame.step(interp),
            Self::AbstractBranch(frame) => frame.step(interp),
            Self::Block(frame) => frame.step(interp),
            Self::Region(frame) => frame.step(interp),
            Self::Call(frame) => frame.step(interp),
            Self::Function(frame) => frame.step(interp),
            Self::StagedFunction(frame) => frame.step(interp),
            Self::SpecializedFunction(frame) => frame.step(interp),
        }
    }

    fn resume_done(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self {
            Self::Statement(frame) => frame.resume_done(interp),
            Self::AbstractBranch(frame) => frame.resume_done(interp),
            Self::Block(frame) => frame.resume_done(interp),
            Self::Region(frame) => frame.resume_done(interp),
            Self::Call(frame) => frame.resume_done(interp),
            Self::Function(frame) => frame.resume_done(interp),
            Self::StagedFunction(frame) => frame.resume_done(interp),
            Self::SpecializedFunction(frame) => frame.resume_done(interp),
        }
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self {
            Self::Statement(frame) => frame.resume(completion, interp),
            Self::AbstractBranch(frame) => frame.resume(completion, interp),
            Self::Block(frame) => frame.resume(completion, interp),
            Self::Region(frame) => frame.resume(completion, interp),
            Self::Call(frame) => frame.resume(completion, interp),
            Self::Function(frame) => frame.resume(completion, interp),
            Self::StagedFunction(frame) => frame.resume(completion, interp),
            Self::SpecializedFunction(frame) => frame.resume(completion, interp),
        }
    }
}
