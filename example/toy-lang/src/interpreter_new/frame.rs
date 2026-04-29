use kirin::prelude::Dialect;
use kirin_arith::ArithType;
use kirin_interpreter_new::{
    BlockFrame, CallFrame, Frame, FrameEffect, FunctionFrame, HasLocation, Location, RegionFrame,
    SpecializedFunctionFrame, StagedFunctionFrame, StandardFrame, StatementFrame,
};
use kirin_scf::interpreter_new::{ForFrame, IfFrame, ScfFrame};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToyFrame<L: Dialect, V> {
    Standard(StandardFrame<L, V>),
    Scf(ScfFrame<L, ArithType, V>),
}

impl<L: Dialect, V> From<StandardFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: StandardFrame<L, V>) -> Self {
        Self::Standard(frame)
    }
}

impl<L: Dialect, V> From<StatementFrame> for ToyFrame<L, V> {
    fn from(frame: StatementFrame) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<BlockFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: BlockFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<RegionFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: RegionFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<CallFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: CallFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<FunctionFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: FunctionFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<StagedFunctionFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: StagedFunctionFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<SpecializedFunctionFrame<L, V>> for ToyFrame<L, V> {
    fn from(frame: SpecializedFunctionFrame<L, V>) -> Self {
        Self::Standard(frame.into())
    }
}

impl<L: Dialect, V> From<ScfFrame<L, ArithType, V>> for ToyFrame<L, V> {
    fn from(frame: ScfFrame<L, ArithType, V>) -> Self {
        Self::Scf(frame)
    }
}

impl<L: Dialect, V> From<IfFrame<L, ArithType, V>> for ToyFrame<L, V> {
    fn from(frame: IfFrame<L, ArithType, V>) -> Self {
        Self::Scf(frame.into())
    }
}

impl<L: Dialect, V> From<ForFrame<L, ArithType, V>> for ToyFrame<L, V> {
    fn from(frame: ForFrame<L, ArithType, V>) -> Self {
        Self::Scf(frame.into())
    }
}

impl<L: Dialect, V> HasLocation for ToyFrame<L, V> {
    fn location(&self) -> Location {
        match self {
            Self::Standard(frame) => frame.location(),
            Self::Scf(frame) => frame.location(),
        }
    }
}

impl<I, L, C, E, V> Frame<I, ToyFrame<L, V>, C, E> for ToyFrame<L, V>
where
    L: Dialect,
    StandardFrame<L, V>: Frame<I, ToyFrame<L, V>, C, E>,
    ScfFrame<L, ArithType, V>: Frame<I, ToyFrame<L, V>, C, E>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<ToyFrame<L, V>, C>, E> {
        match self {
            Self::Standard(frame) => frame.step(interp),
            Self::Scf(frame) => frame.step(interp),
        }
    }

    fn resume_done(self, interp: &mut I) -> Result<FrameEffect<ToyFrame<L, V>, C>, E> {
        match self {
            Self::Standard(frame) => frame.resume_done(interp),
            Self::Scf(frame) => frame.resume_done(interp),
        }
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<ToyFrame<L, V>, C>, E> {
        match self {
            Self::Standard(frame) => frame.resume(completion, interp),
            Self::Scf(frame) => frame.resume(completion, interp),
        }
    }
}
