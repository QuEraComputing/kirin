mod arena;
mod builder;
mod comptime;
mod context;
mod detach;
mod intern;
mod language;
mod lattice;
mod node;
mod visitor;

/// Queries from the IRContext.
pub mod query;

pub use arena::GetInfo;
pub use comptime::{CompileTimeValue, Typeof};
pub use context::Context;
pub use detach::Detach;
pub use intern::InternTable;
pub use language::{
    Dialect, HasName, HasArguments, HasArgumentsMut, HasBlocks, HasBlocksMut, HasRegions, HasRegionsMut,
    HasResults, HasResultsMut, HasSuccessors, HasSuccessorsMut, IsConstant, IsPure, IsTerminator,
};
pub use lattice::{FiniteLattice, Lattice, TypeLattice};
pub use node::{
    Block, BlockArgument, BlockInfo, CompileStage, DeletedSSAValue, Function, FunctionInfo,
    LinkedList, LinkedListNode, Region, ResultValue, SSAInfo, SSAKind, SSAValue, Signature,
    SpecializedFunction, SpecializedFunctionInfo, StagedFunction, StagedFunctionInfo, Statement,
    StatementInfo, Successor, Symbol, TestSSAValue,
};

#[cfg(feature = "derive")]
pub use kirin_derive::{
    Dialect, HasArguments, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure, IsTerminator,
};
