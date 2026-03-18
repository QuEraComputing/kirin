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
pub use builder::error::{
    PipelineError, PipelineStagedError, SpecializeError, StagedFunctionConflictKind,
    StagedFunctionError,
};
pub use builder::{BuilderStageInfo, FinalizeError};
pub use comptime::{CompileTimeValue, Placeholder, Typeof};
pub use context::StageInfo;
pub use detach::Detach;
pub use intern::InternTable;
pub use language::{
    Dialect, HasArguments, HasArgumentsMut, HasBlocks, HasBlocksMut, HasDigraphs, HasDigraphsMut,
    HasRegions, HasRegionsMut, HasResults, HasResultsMut, HasSuccessors, HasSuccessorsMut,
    HasUngraphs, HasUngraphsMut, IsConstant, IsEdge, IsPure, IsSpeculatable, IsTerminator,
};
pub use lattice::{FiniteLattice, HasBottom, HasTop, Lattice, TypeLattice};
pub use node::{
    Block, BlockArgument, BlockInfo, BuilderKey, BuilderSSAKind, CompileStage, DeletedSSAValue,
    DiGraph, DiGraphInfo, Function, FunctionInfo, GlobalSymbol, LinkedList, LinkedListNode, Port,
    PortParent, Region, ResolutionInfo, ResultValue, SSAInfo, SSAKind, SSAValue,
    SpecializedFunction, SpecializedFunctionInfo, StagedFunction, StagedFunctionInfo,
    StagedNamePolicy, Statement, StatementInfo, StatementParent, Successor, Symbol, TestSSAValue,
    UnGraph, UnGraphInfo,
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
        Block, BuilderStageInfo, CompileStage, Dialect, Function, GetInfo, HasStageInfo, Pipeline,
        Region, ResultValue, SSAValue, Signature, SignatureSemantics, StageInfo, StageMeta,
        Statement,
    };
    pub use crate::{CompileTimeValue, Placeholder, Typeof};
}

#[cfg(feature = "derive")]
pub use kirin_derive_ir::{
    Dialect, HasArguments, HasDigraphs, HasRegions, HasResults, HasSuccessors, HasUngraphs,
    IsConstant, IsEdge, IsPure, IsSpeculatable, IsTerminator, ParseDispatch, StageMeta,
};
