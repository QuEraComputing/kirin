mod arena;
mod builder;
mod comptime;
mod context;
mod detach;
mod intern;
mod language;
mod lattice;
mod node;
mod pipeline;
mod signature;
mod stage;

/// Queries from the IRContext.
pub mod query;

pub use arena::{Arena, DenseHint, GetInfo, Id, Identifier, Item, SparseHint};
pub use builder::error::{SpecializeError, StagedFunctionConflictKind, StagedFunctionError};
pub use comptime::{CompileTimeValue, Typeof};
pub use context::StageInfo;
pub use detach::Detach;
pub use intern::InternTable;
pub use language::{
    Dialect, HasArguments, HasArgumentsMut, HasBlocks, HasBlocksMut, HasRegions, HasRegionsMut,
    HasResults, HasResultsMut, HasSuccessors, HasSuccessorsMut, IsConstant, IsPure, IsSpeculatable,
    IsTerminator,
};
pub use lattice::{FiniteLattice, HasBottom, HasTop, Lattice, TypeLattice};
pub use node::{
    Block, BlockArgument, BlockInfo, CompileStage, DeletedSSAValue, Function, FunctionInfo,
    GlobalSymbol, LinkedList, LinkedListNode, Region, ResultValue, SSAInfo, SSAKind, SSAValue,
    SpecializedFunction, SpecializedFunctionInfo, StagedFunction, StagedFunctionInfo,
    StagedNamePolicy, Statement, StatementInfo, Successor, Symbol, TestSSAValue,
};
pub use pipeline::Pipeline;
pub use signature::{
    ExactSemantics, LatticeSemantics, Signature, SignatureCmp, SignatureSemantics,
};
pub use stage::{
    HasStageInfo, StageAction, StageActionMut, StageDispatch, StageDispatchMiss, StageDispatchMut,
    StageDispatchRequiredError, StageMeta, SupportsStageDispatch, SupportsStageDispatchMut,
};

/// Re-exports of the most commonly used types for dialect authors.
pub mod prelude {
    pub use crate::{
        Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, Region, ResultValue,
        SSAValue, StageInfo, StageMeta, Statement,
    };
    pub use crate::{CompileTimeValue, Typeof};
}

#[cfg(feature = "derive")]
pub use kirin_derive::{
    Dialect, HasArguments, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure,
    IsSpeculatable, IsTerminator, StageMeta,
};
