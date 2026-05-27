pub mod abstract_interp;
pub mod concrete;
pub mod dispatch;
pub mod effect;
pub mod env;
pub mod error;
pub mod frame;
pub mod location;
#[macro_use]
mod macros;
pub mod standard;
pub mod value;

pub use abstract_interp::{
    AbstractEnv, AbstractEnvStore, AbstractInterpreter, AbstractInterpreterWithStore,
    AbstractValue, BackwardSummaryDeps, ContextStrategy, EnvSummary, FixpointPhase,
    ForwardSummaryDeps, NodeContext, OwnerSemantics, OwnerSummaryDeps, SimpleFixpointInterpreter,
    StandardFixpointInterpreter, Summary, SummaryDependency, SummaryDependencyIndex, SummaryEffect,
    SummaryKey, WidenNarrowStrategy, WorkItem,
};
pub use concrete::{ConcreteInterpreter, StepResult};
pub use dispatch::{Interpretable, StageAccess, StatementDispatch};
pub use effect::{
    AbstractBlockTransfer, BlockTransfer, ConcreteBlockTransfer, FrameEffect, StandardCompletion,
    StatementEffect, expect_single_function_return,
};
pub use env::{Env, EnvIndex, EnvStackStore, ForkEnv};
pub use error::InterpreterError;
pub use frame::{Frame, HasLocation, ProjectOrSelf};
pub use location::{Location, Position, Traversal};
pub use standard::{
    AbstractBranchFrame, AbstractBranchState, BlockFrame, BlockTransferDispatch, CallFrame, Callee,
    FunctionAccess, FunctionBodyDispatch, FunctionEntry, FunctionEntryTarget, FunctionFrame,
    FunctionInvocation, FunctionInvocationDispatch, FunctionInvocationFrame, FunctionInvokeBuilder,
    FunctionInvokeTargetBuilder, RegionFrame, SpecializedFunctionFrame, SpecializedFunctionState,
    StageBlockDispatch, StagedFunctionFrame, StandardFrame, StatementFrame,
};
pub use value::{BranchCondition, HasProductValue};

#[cfg(feature = "derive")]
pub use kirin_derive_interpreter_new::{
    Completion, Frame, FunctionEntry, HasLocation, Interpretable, LiftError,
};
