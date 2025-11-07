mod comptime;
mod context;
mod node;
mod language;
mod lattice;

pub use comptime::CompileTimeValue;
pub use context::{Context, IRContext};
pub use language::{Language, Instruction};
pub use lattice::{Lattice, FiniteLattice, TypeLattice};
pub use node::{
    SSAKind, SSAValue, SSAInfo, TestSSAValue, ResultValue, BlockArgument,
    Statement, StatementInfo, Block, BlockInfo,
    CFG,
    Function, FunctionInfo, Signature, SpecializedFunction, SpecializedFunctionInfo,
    StagedFunction, StagedFunctionInfo,
    Module, SpecializedModule,
    InternTable, Symbol,
    LinkedList, Node,
};
#[cfg(feature = "derive")]
pub use kirin_derive::Instruction;
