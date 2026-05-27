use core::convert::Infallible;

use kirin::prelude::{Dialect, TryLift, TryLiftFrom};
use kirin_arith::ArithType;
use kirin_interpreter_new::{
    CallFrame, ConcreteBlockTransfer, Frame, FunctionFrame, FunctionInvocation,
    FunctionInvocationFrame, HasLocation, SpecializedFunctionFrame, StagedFunctionFrame,
    StandardFrame,
};
use kirin_scf::interpreter_new::ScfFrame;

use crate::language::{HighLevel, LowLevel};

#[derive(Clone, Debug, PartialEq, Eq, HasLocation, Frame)]
pub enum ToyFrame<L: Dialect, V, T = ConcreteBlockTransfer<V>> {
    Standard(StandardFrame<L, V, T>),
    Scf(ScfFrame<L, ArithType, V, T>),
}

impl<L: Dialect, V, T> FunctionInvocationFrame<V> for ToyFrame<L, V, T> {
    type Language = L;
    type Error = Infallible;

    fn from_function_invocation(invocation: FunctionInvocation<V>) -> Result<Self, Self::Error> {
        invocation
            .into_root_frame::<L, StandardFrame<L, V, T>, Self::Error>()?
            .try_lift()
    }
}

impl<L: Dialect, V, T> TryLiftFrom<CallFrame<L, V>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: CallFrame<L, V>) -> Result<Self, Self::Error> {
        Ok(Self::Standard(StandardFrame::Call(frame)))
    }
}

impl<L: Dialect, V, T> TryLiftFrom<FunctionFrame<L, V>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: FunctionFrame<L, V>) -> Result<Self, Self::Error> {
        Ok(Self::Standard(StandardFrame::Function(frame)))
    }
}

impl<L: Dialect, V, T> TryLiftFrom<StagedFunctionFrame<L, V>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: StagedFunctionFrame<L, V>) -> Result<Self, Self::Error> {
        Ok(Self::Standard(StandardFrame::StagedFunction(frame)))
    }
}

impl<L: Dialect, V, T> TryLiftFrom<SpecializedFunctionFrame<L, V>> for ToyFrame<L, V, T> {
    type Error = Infallible;

    fn try_lift_from(frame: SpecializedFunctionFrame<L, V>) -> Result<Self, Self::Error> {
        Ok(Self::Standard(StandardFrame::SpecializedFunction(frame)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, HasLocation, Frame)]
pub enum ToyStageFrame<V, T = ConcreteBlockTransfer<V>> {
    Source(ToyFrame<HighLevel, V, T>),
    Lowered(ToyFrame<LowLevel, V, T>),
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
impl_stage_lift!(LowLevel, Lowered, ScfFrame<LowLevel, ArithType, V, T>);
