mod comptime;
mod context;
mod detach;
mod language;
mod lattice;
mod node;
/// Queries from the IRContext.
pub mod query;

pub use comptime::CompileTimeValue;
pub use context::{Context, IRContext};
pub use detach::Detach;
pub use language::{
    HasArguments, HasRegions, HasResults, HasSuccessors, Instruction, IsConstant, IsPure,
    IsTerminator, Language,
};
pub use lattice::{FiniteLattice, Lattice, TypeLattice};
pub use node::{
    Block, BlockArgument, BlockInfo, Function, FunctionInfo, InternTable, LinkedList, Module, Node,
    Region, ResultValue, SSAInfo, SSAKind, SSAValue, Signature, SpecializedFunction,
    SpecializedFunctionInfo, SpecializedModule, StagedFunction, StagedFunctionInfo, Statement,
    StatementInfo, Symbol, TestSSAValue,
};

#[cfg(feature = "derive")]
pub use kirin_derive::{
    HasArguments, HasRegions, HasResults, HasSuccessors, Instruction, IsConstant, IsPure,
    IsTerminator,
};
