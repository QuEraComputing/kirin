mod comptime;
mod arena;
mod detach;
mod intern;
mod language;
mod lattice;
mod node;
/// Queries from the IRContext.
pub mod query;
pub mod context;

pub use context::Context;
pub use comptime::CompileTimeValue;
pub use arena::Arena;
pub use detach::Detach;
pub use intern::InternTable;
pub use language::{
    HasArguments, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure, IsTerminator,
    Language, Statement,
};
pub use lattice::{FiniteLattice, Lattice, TypeLattice};
pub use node::{
    Block, BlockArgument, BlockInfo, CompileStage, Function, FunctionInfo, LinkedList, Module,
    LinkedListNode, Region, ResultValue, SSAInfo, SSAKind, SSAValue, Signature, SpecializedFunction,
    SpecializedFunctionInfo, SpecializedModule, StagedFunction, StagedFunctionInfo, StatementInfo,
    StatementRef, Symbol, TestSSAValue,
};

#[cfg(feature = "derive")]
pub use kirin_derive::{
    HasArguments, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure, IsTerminator,
    Statement
};
