use crate::{Frame, FrameEffect, HasLocation, Location};

use super::{
    BlockFrame, CallFrame, FunctionFrame, RegionFrame, SpecializedFunctionFrame,
    StagedFunctionFrame, StatementFrame,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StandardFrame<L, V> {
    Statement(StatementFrame),
    Block(BlockFrame<L, V>),
    Region(RegionFrame<L, V>),
    Call(CallFrame<L, V>),
    Function(FunctionFrame<L, V>),
    StagedFunction(StagedFunctionFrame<L, V>),
    SpecializedFunction(SpecializedFunctionFrame<L, V>),
}

impl<L, V> From<StatementFrame> for StandardFrame<L, V> {
    fn from(frame: StatementFrame) -> Self {
        Self::Statement(frame)
    }
}

impl<L, V> From<BlockFrame<L, V>> for StandardFrame<L, V> {
    fn from(frame: BlockFrame<L, V>) -> Self {
        Self::Block(frame)
    }
}

impl<L, V> From<RegionFrame<L, V>> for StandardFrame<L, V> {
    fn from(frame: RegionFrame<L, V>) -> Self {
        Self::Region(frame)
    }
}

impl<L, V> From<CallFrame<L, V>> for StandardFrame<L, V> {
    fn from(frame: CallFrame<L, V>) -> Self {
        Self::Call(frame)
    }
}

impl<L, V> From<FunctionFrame<L, V>> for StandardFrame<L, V> {
    fn from(frame: FunctionFrame<L, V>) -> Self {
        Self::Function(frame)
    }
}

impl<L, V> From<StagedFunctionFrame<L, V>> for StandardFrame<L, V> {
    fn from(frame: StagedFunctionFrame<L, V>) -> Self {
        Self::StagedFunction(frame)
    }
}

impl<L, V> From<SpecializedFunctionFrame<L, V>> for StandardFrame<L, V> {
    fn from(frame: SpecializedFunctionFrame<L, V>) -> Self {
        Self::SpecializedFunction(frame)
    }
}

impl<L, V> HasLocation for StandardFrame<L, V> {
    fn location(&self) -> Location {
        match self {
            Self::Statement(frame) => frame.location(),
            Self::Block(frame) => frame.location(),
            Self::Region(frame) => frame.location(),
            Self::Call(frame) => frame.location(),
            Self::Function(frame) => frame.location(),
            Self::StagedFunction(frame) => frame.location(),
            Self::SpecializedFunction(frame) => frame.location(),
        }
    }
}

impl<I, L, C, E, V> Frame<I, StandardFrame<L, V>, C, E> for StandardFrame<L, V>
where
    StatementFrame: Frame<I, StandardFrame<L, V>, C, E>,
    BlockFrame<L, V>: Frame<I, StandardFrame<L, V>, C, E>,
    RegionFrame<L, V>: Frame<I, StandardFrame<L, V>, C, E>,
    CallFrame<L, V>: Frame<I, StandardFrame<L, V>, C, E>,
    FunctionFrame<L, V>: Frame<I, StandardFrame<L, V>, C, E>,
    StagedFunctionFrame<L, V>: Frame<I, StandardFrame<L, V>, C, E>,
    SpecializedFunctionFrame<L, V>: Frame<I, StandardFrame<L, V>, C, E>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<StandardFrame<L, V>, C>, E> {
        match self {
            Self::Statement(frame) => frame.step(interp),
            Self::Block(frame) => frame.step(interp),
            Self::Region(frame) => frame.step(interp),
            Self::Call(frame) => frame.step(interp),
            Self::Function(frame) => frame.step(interp),
            Self::StagedFunction(frame) => frame.step(interp),
            Self::SpecializedFunction(frame) => frame.step(interp),
        }
    }

    fn resume_done(self, interp: &mut I) -> Result<FrameEffect<StandardFrame<L, V>, C>, E> {
        match self {
            Self::Statement(frame) => frame.resume_done(interp),
            Self::Block(frame) => frame.resume_done(interp),
            Self::Region(frame) => frame.resume_done(interp),
            Self::Call(frame) => frame.resume_done(interp),
            Self::Function(frame) => frame.resume_done(interp),
            Self::StagedFunction(frame) => frame.resume_done(interp),
            Self::SpecializedFunction(frame) => frame.resume_done(interp),
        }
    }

    fn resume(
        self,
        completion: C,
        interp: &mut I,
    ) -> Result<FrameEffect<StandardFrame<L, V>, C>, E> {
        match self {
            Self::Statement(frame) => frame.resume(completion, interp),
            Self::Block(frame) => frame.resume(completion, interp),
            Self::Region(frame) => frame.resume(completion, interp),
            Self::Call(frame) => frame.resume(completion, interp),
            Self::Function(frame) => frame.resume(completion, interp),
            Self::StagedFunction(frame) => frame.resume(completion, interp),
            Self::SpecializedFunction(frame) => frame.resume(completion, interp),
        }
    }
}
