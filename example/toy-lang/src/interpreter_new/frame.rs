use core::convert::Infallible;

use kirin::prelude::{Dialect, TryLift, TryLiftFrom};
use kirin_arith::ArithType;
use kirin_interpreter_new::{
    AbstractBranchFrame, BlockFrame, CallFrame, ConcreteBlockTransfer, Frame, FrameEffect,
    FunctionFrame, HasLocation, Location, RegionFrame, SpecializedFunctionFrame,
    StagedFunctionFrame, StandardFrame, StatementFrame,
};
use kirin_scf::interpreter_new::{ForFrame, IfFrame, ScfFrame};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToyFrame<L: Dialect, V, T = ConcreteBlockTransfer<V>> {
    Standard(StandardFrame<L, V, T>),
    Scf(ScfFrame<L, ArithType, V, T>),
}

impl<L: Dialect, V, T> TryLiftFrom<StandardFrame<L, V, T>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: StandardFrame<L, V, T>) -> Result<Self, Self::Error> {
        Ok(Self::Standard(frame))
    }
}

impl<L: Dialect, V, T> TryLiftFrom<StatementFrame> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: StatementFrame) -> Result<Self, Self::Error> {
        frame.try_lift().map(Self::Standard)
    }
}

impl<L: Dialect, V, T> TryLiftFrom<AbstractBranchFrame<L, V>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: AbstractBranchFrame<L, V>) -> Result<Self, Self::Error> {
        frame.try_lift().map(Self::Standard)
    }
}

impl<L: Dialect, V, T> TryLiftFrom<BlockFrame<L, V, T>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: BlockFrame<L, V, T>) -> Result<Self, Self::Error> {
        frame.try_lift().map(Self::Standard)
    }
}

impl<L: Dialect, V, T> TryLiftFrom<RegionFrame<L, V, T>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: RegionFrame<L, V, T>) -> Result<Self, Self::Error> {
        frame.try_lift().map(Self::Standard)
    }
}

impl<L: Dialect, V, T> TryLiftFrom<CallFrame<L, V>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: CallFrame<L, V>) -> Result<Self, Self::Error> {
        frame.try_lift().map(Self::Standard)
    }
}

impl<L: Dialect, V, T> TryLiftFrom<FunctionFrame<L, V>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: FunctionFrame<L, V>) -> Result<Self, Self::Error> {
        frame.try_lift().map(Self::Standard)
    }
}

impl<L: Dialect, V, T> TryLiftFrom<StagedFunctionFrame<L, V>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: StagedFunctionFrame<L, V>) -> Result<Self, Self::Error> {
        frame.try_lift().map(Self::Standard)
    }
}

impl<L: Dialect, V, T> TryLiftFrom<SpecializedFunctionFrame<L, V>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: SpecializedFunctionFrame<L, V>) -> Result<Self, Self::Error> {
        frame.try_lift().map(Self::Standard)
    }
}

impl<L: Dialect, V, T> TryLiftFrom<ScfFrame<L, ArithType, V, T>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: ScfFrame<L, ArithType, V, T>) -> Result<Self, Self::Error> {
        Ok(Self::Scf(frame))
    }
}

impl<L: Dialect, V, T> TryLiftFrom<IfFrame<L, ArithType, V, T>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: IfFrame<L, ArithType, V, T>) -> Result<Self, Self::Error> {
        frame.try_lift().map(Self::Scf)
    }
}

impl<L: Dialect, V, T> TryLiftFrom<ForFrame<L, ArithType, V, T>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: ForFrame<L, ArithType, V, T>) -> Result<Self, Self::Error> {
        frame.try_lift().map(Self::Scf)
    }
}

impl<L: Dialect, V, T> HasLocation for ToyFrame<L, V, T> {
    fn location(&self) -> Location {
        match self {
            Self::Standard(frame) => frame.location(),
            Self::Scf(frame) => frame.location(),
        }
    }
}

impl<I, L, C, E, V, T> Frame<I, ToyFrame<L, V, T>, C, E> for ToyFrame<L, V, T>
where
    L: Dialect,
    StandardFrame<L, V, T>: Frame<I, ToyFrame<L, V, T>, C, E>,
    ScfFrame<L, ArithType, V, T>: Frame<I, ToyFrame<L, V, T>, C, E>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<ToyFrame<L, V, T>, C>, E> {
        match self {
            Self::Standard(frame) => frame.step(interp),
            Self::Scf(frame) => frame.step(interp),
        }
    }

    fn resume_done(self, interp: &mut I) -> Result<FrameEffect<ToyFrame<L, V, T>, C>, E> {
        match self {
            Self::Standard(frame) => frame.resume_done(interp),
            Self::Scf(frame) => frame.resume_done(interp),
        }
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<ToyFrame<L, V, T>, C>, E> {
        match self {
            Self::Standard(frame) => frame.resume(completion, interp),
            Self::Scf(frame) => frame.resume(completion, interp),
        }
    }
}
