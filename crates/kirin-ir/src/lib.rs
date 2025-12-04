mod arena;
mod context;
mod builder;
mod comptime;
mod detach;
mod intern;
mod language;
mod lattice;
mod node;
mod visitor;

/// Queries from the IRContext.
pub mod query;

#[cfg(test)]
pub mod tests;

pub use context::Context;
pub use comptime::{CompileTimeValue, Typeof};
pub use detach::Detach;
pub use intern::InternTable;
pub use language::{
    HasArguments, HasArgumentsMut, HasRegions, HasRegionsMut, HasResults, HasResultsMut,
    HasSuccessors, HasSuccessorsMut, IsConstant, IsPure, IsTerminator, Dialect,
};
pub use lattice::{FiniteLattice, Lattice, TypeLattice};
pub use node::{
    Block, BlockArgument, BlockInfo, CompileStage, Function, FunctionInfo, LinkedList,
    LinkedListNode, Region, ResultValue, SSAInfo, SSAKind, SSAValue, Signature,
    SpecializedFunction, SpecializedFunctionInfo, StagedFunction, StagedFunctionInfo, Statement,
    StatementInfo, Symbol, TestSSAValue,
};

#[cfg(feature = "derive")]
pub use kirin_derive::{
    HasArguments, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure, IsTerminator,
    Dialect,
};
