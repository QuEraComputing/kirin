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
mod stage_dispatch;

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
pub use pipeline::{HasStageInfo, Pipeline, StageMeta};
pub use signature::{
    ExactSemantics, LatticeSemantics, Signature, SignatureCmp, SignatureSemantics,
};
pub use stage_dispatch::{StageAction, StageActionMut, StageDispatch, StageDispatchMut};

#[cfg(feature = "derive")]
pub use kirin_derive::{
    Dialect, HasArguments, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure,
    IsSpeculatable, IsTerminator, StageMeta,
};
