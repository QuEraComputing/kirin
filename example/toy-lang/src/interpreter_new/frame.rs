use core::convert::Infallible;

use kirin::prelude::Dialect;
use kirin_arith::ArithType;
use kirin_interpreter_new::{
    AbstractBranchFrame, BlockFrame, CallFrame, ConcreteBlockTransfer, Frame, FunctionFrame,
    FunctionInvocation, FunctionInvocationFrame, HasLocation, RegionFrame,
    SpecializedFunctionFrame, StagedFunctionFrame, StandardFrame, StatementFrame, forward_through,
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

forward_through! {
    impl[L: Dialect, V, T] for [ToyFrame<L, V, T>] via [StandardFrame<L, V, T>]
    from {
        StatementFrame,
        AbstractBranchFrame<L, V>,
        BlockFrame<L, V, T>,
        RegionFrame<L, V, T>,
        CallFrame<L, V>,
        FunctionFrame<L, V>,
        StagedFunctionFrame<L, V>,
        SpecializedFunctionFrame<L, V>,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, HasLocation, Frame)]
pub enum ToyStageFrame<V, T = ConcreteBlockTransfer<V>> {
    Source(ToyFrame<HighLevel, V, T>),
    Lowered(ToyFrame<LowLevel, V, T>),
}

// StatementFrame is intentionally omitted from the ToyStageFrame lifts: it has no
// language tag, so a direct lift would be ambiguous between Source and Lowered.
// Callers must pick a stage by routing through ToyFrame<HighLevel> or ToyFrame<LowLevel>.

forward_through! {
    impl[V, T] for [ToyStageFrame<V, T>] via [ToyFrame<HighLevel, V, T>]
    from {
        StandardFrame<HighLevel, V, T>,
        ScfFrame<HighLevel, ArithType, V, T>,
        AbstractBranchFrame<HighLevel, V>,
        BlockFrame<HighLevel, V, T>,
        RegionFrame<HighLevel, V, T>,
        CallFrame<HighLevel, V>,
        FunctionFrame<HighLevel, V>,
        StagedFunctionFrame<HighLevel, V>,
        SpecializedFunctionFrame<HighLevel, V>,
    }
}

forward_through! {
    impl[V, T] for [ToyStageFrame<V, T>] via [ToyFrame<LowLevel, V, T>]
    from {
        StandardFrame<LowLevel, V, T>,
        ScfFrame<LowLevel, ArithType, V, T>,
        AbstractBranchFrame<LowLevel, V>,
        BlockFrame<LowLevel, V, T>,
        RegionFrame<LowLevel, V, T>,
        CallFrame<LowLevel, V>,
        FunctionFrame<LowLevel, V>,
        StagedFunctionFrame<LowLevel, V>,
        SpecializedFunctionFrame<LowLevel, V>,
    }
}
