use core::convert::Infallible;

use kirin::prelude::{Dialect, TryLift, TryLiftFrom};
use kirin_arith::ArithType;
use kirin_interpreter_new::{
    AbstractBranchFrame, BlockFrame, CallFrame, ConcreteBlockTransfer, Frame, FrameEffect,
    FunctionFrame, FunctionInvocation, FunctionInvocationFrame, HasLocation, RegionFrame,
    SpecializedFunctionFrame, StagedFunctionFrame, StandardFrame, StatementFrame,
};
use kirin_scf::interpreter_new::{ForFrame, IfFrame, ScfFrame};

use crate::language::{HighLevel, LowLevel};

#[derive(Clone, Debug, PartialEq, Eq, HasLocation, Frame)]
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

impl<L: Dialect, V, T> FunctionInvocationFrame<V> for ToyFrame<L, V, T> {
    type Language = L;
    type Error = Infallible;

    fn from_function_invocation(invocation: FunctionInvocation<V>) -> Result<Self, Self::Error> {
        invocation.into_root_frame::<L, Self, Self::Error>()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, HasLocation)]
pub enum ToyStageFrame<V, T = ConcreteBlockTransfer<V>> {
    Source(ToyFrame<HighLevel, V, T>),
    Lowered(ToyFrame<LowLevel, V, T>),
}

impl<V, T> TryLiftFrom<ToyFrame<HighLevel, V, T>> for ToyStageFrame<V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: ToyFrame<HighLevel, V, T>) -> Result<Self, Self::Error> {
        Ok(Self::Source(frame))
    }
}

impl<V, T> TryLiftFrom<ToyFrame<LowLevel, V, T>> for ToyStageFrame<V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: ToyFrame<LowLevel, V, T>) -> Result<Self, Self::Error> {
        Ok(Self::Lowered(frame))
    }
}

macro_rules! impl_stage_lift {
    ($language:ty, $variant:ident, $frame:ty) => {
        impl<V, T> TryLiftFrom<$frame> for ToyStageFrame<V, T> {
            type Error = Infallible;

            fn try_lift_from(frame: $frame) -> Result<Self, Self::Error> {
                frame.try_lift().map(Self::$variant)
            }
        }
    };
}

impl_stage_lift!(
    HighLevel,
    Source,
    StandardFrame<HighLevel, V, T>
);
impl_stage_lift!(
    LowLevel,
    Lowered,
    StandardFrame<LowLevel, V, T>
);
impl_stage_lift!(HighLevel, Source, StatementFrame);
impl_stage_lift!(HighLevel, Source, AbstractBranchFrame<HighLevel, V>);
impl_stage_lift!(LowLevel, Lowered, AbstractBranchFrame<LowLevel, V>);
impl_stage_lift!(HighLevel, Source, BlockFrame<HighLevel, V, T>);
impl_stage_lift!(LowLevel, Lowered, BlockFrame<LowLevel, V, T>);
impl_stage_lift!(HighLevel, Source, RegionFrame<HighLevel, V, T>);
impl_stage_lift!(LowLevel, Lowered, RegionFrame<LowLevel, V, T>);
impl_stage_lift!(HighLevel, Source, CallFrame<HighLevel, V>);
impl_stage_lift!(LowLevel, Lowered, CallFrame<LowLevel, V>);
impl_stage_lift!(HighLevel, Source, FunctionFrame<HighLevel, V>);
impl_stage_lift!(LowLevel, Lowered, FunctionFrame<LowLevel, V>);
impl_stage_lift!(HighLevel, Source, StagedFunctionFrame<HighLevel, V>);
impl_stage_lift!(LowLevel, Lowered, StagedFunctionFrame<LowLevel, V>);
impl_stage_lift!(
    HighLevel,
    Source,
    SpecializedFunctionFrame<HighLevel, V>
);
impl_stage_lift!(
    LowLevel,
    Lowered,
    SpecializedFunctionFrame<LowLevel, V>
);
impl_stage_lift!(HighLevel, Source, ScfFrame<HighLevel, ArithType, V, T>);
impl_stage_lift!(HighLevel, Source, IfFrame<HighLevel, ArithType, V, T>);
impl_stage_lift!(HighLevel, Source, ForFrame<HighLevel, ArithType, V, T>);

impl<I, C, E, V, T> Frame<I, ToyStageFrame<V, T>, C, E> for ToyStageFrame<V, T>
where
    StandardFrame<HighLevel, V, T>: Frame<I, ToyStageFrame<V, T>, C, E>,
    ScfFrame<HighLevel, ArithType, V, T>: Frame<I, ToyStageFrame<V, T>, C, E>,
    StandardFrame<LowLevel, V, T>: Frame<I, ToyStageFrame<V, T>, C, E>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<ToyStageFrame<V, T>, C>, E> {
        match self {
            Self::Source(ToyFrame::Standard(frame)) => frame.step(interp),
            Self::Source(ToyFrame::Scf(frame)) => frame.step(interp),
            Self::Lowered(ToyFrame::Standard(frame)) => frame.step(interp),
            Self::Lowered(ToyFrame::Scf(_)) => unreachable!("low-level toy frames do not use scf"),
        }
    }

    fn resume_done(self, interp: &mut I) -> Result<FrameEffect<ToyStageFrame<V, T>, C>, E> {
        match self {
            Self::Source(ToyFrame::Standard(frame)) => frame.resume_done(interp),
            Self::Source(ToyFrame::Scf(frame)) => frame.resume_done(interp),
            Self::Lowered(ToyFrame::Standard(frame)) => frame.resume_done(interp),
            Self::Lowered(ToyFrame::Scf(_)) => unreachable!("low-level toy frames do not use scf"),
        }
    }

    fn resume(
        self,
        completion: C,
        interp: &mut I,
    ) -> Result<FrameEffect<ToyStageFrame<V, T>, C>, E> {
        match self {
            Self::Source(ToyFrame::Standard(frame)) => frame.resume(completion, interp),
            Self::Source(ToyFrame::Scf(frame)) => frame.resume(completion, interp),
            Self::Lowered(ToyFrame::Standard(frame)) => frame.resume(completion, interp),
            Self::Lowered(ToyFrame::Scf(_)) => unreachable!("low-level toy frames do not use scf"),
        }
    }
}
