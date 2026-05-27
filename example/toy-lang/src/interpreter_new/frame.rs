use core::convert::Infallible;

use kirin::prelude::Dialect;
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
        Ok(invocation
            .into_root_frame::<L, StandardFrame<L, V, T>, Self::Error>()?
            .into())
    }
}

impl<L: Dialect, V, T> From<CallFrame<L, V>> for ToyFrame<L, V, T> {
    fn from(frame: CallFrame<L, V>) -> Self {
        Self::Standard(StandardFrame::Call(frame))
    }
}

impl<L: Dialect, V, T> From<FunctionFrame<L, V>> for ToyFrame<L, V, T> {
    fn from(frame: FunctionFrame<L, V>) -> Self {
        Self::Standard(StandardFrame::Function(frame))
    }
}

impl<L: Dialect, V, T> From<StagedFunctionFrame<L, V>> for ToyFrame<L, V, T> {
    fn from(frame: StagedFunctionFrame<L, V>) -> Self {
        Self::Standard(StandardFrame::StagedFunction(frame))
    }
}

impl<L: Dialect, V, T> From<SpecializedFunctionFrame<L, V>> for ToyFrame<L, V, T> {
    fn from(frame: SpecializedFunctionFrame<L, V>) -> Self {
        Self::Standard(StandardFrame::SpecializedFunction(frame))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, HasLocation, Frame)]
pub enum ToyStageFrame<V, T = ConcreteBlockTransfer<V>> {
    Source(ToyFrame<HighLevel, V, T>),
    Lowered(ToyFrame<LowLevel, V, T>),
}

macro_rules! impl_stage_lift {
    ($variant:ident, $frame:ty) => {
        impl<V, T> From<$frame> for ToyStageFrame<V, T> {
            fn from(frame: $frame) -> Self {
                Self::$variant(frame.into())
            }
        }
    };
}

impl_stage_lift!(Source, StandardFrame<HighLevel, V, T>);
impl_stage_lift!(Lowered, StandardFrame<LowLevel, V, T>);
impl_stage_lift!(Source, CallFrame<HighLevel, V>);
impl_stage_lift!(Lowered, CallFrame<LowLevel, V>);
impl_stage_lift!(Source, FunctionFrame<HighLevel, V>);
impl_stage_lift!(Lowered, FunctionFrame<LowLevel, V>);
impl_stage_lift!(Source, StagedFunctionFrame<HighLevel, V>);
impl_stage_lift!(Lowered, StagedFunctionFrame<LowLevel, V>);
impl_stage_lift!(Source, SpecializedFunctionFrame<HighLevel, V>);
impl_stage_lift!(Lowered, SpecializedFunctionFrame<LowLevel, V>);
impl_stage_lift!(Source, ScfFrame<HighLevel, ArithType, V, T>);
impl_stage_lift!(Lowered, ScfFrame<LowLevel, ArithType, V, T>);
