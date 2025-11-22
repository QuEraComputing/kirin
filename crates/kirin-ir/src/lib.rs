mod arena;
mod comptime;
mod detach;
mod intern;
mod language;
mod lattice;
mod node;
mod builder;
/// Queries from the IRContext.
pub mod query;

#[cfg(test)]
pub mod tests;

pub use arena::Arena;
pub use comptime::{CompileTimeValue, Typeof};
pub use detach::Detach;
pub use intern::InternTable;
pub use language::{
    HasArguments, HasArgumentsMut,
    HasRegions, HasRegionsMut,
    HasResults, HasResultsMut,
    HasSuccessors, HasSuccessorsMut,
    IsConstant, IsPure, IsTerminator,
    Language, Statement,
};
pub use lattice::{FiniteLattice, Lattice, TypeLattice};
pub use node::{
    Block, BlockArgument, BlockInfo, CompileStage, Function, FunctionInfo, LinkedList,
    LinkedListNode, Region, ResultValue, SSAInfo, SSAKind, SSAValue, Signature,
    SpecializedFunction, SpecializedFunctionInfo, StagedFunction, StagedFunctionInfo,
    StatementInfo, StatementId, Symbol, TestSSAValue,
};

#[cfg(feature = "derive")]
pub use kirin_derive::{
    HasArguments, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure, IsTerminator,
    Statement,
};
