#[cfg(feature = "interpret")]
mod interpret_impl;
#[cfg(feature = "interpret")]
pub use interpret_impl::ForLoopValue;

use kirin::ir::*;
use kirin::parsers::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[wraps]
#[kirin(fn, type = T)]
pub enum StructuredControlFlow<T: CompileTimeValue + Default> {
    If(If<T>),
    For(For<T>),
    #[kirin(terminator)]
    Yield(Yield<T>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "if {condition} then {then_body} else {else_body}")]
#[kirin(fn, type = T)]
pub struct If<T: CompileTimeValue + Default> {
    condition: SSAValue,
    then_body: Block,
    else_body: Block,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "for {induction_var} in {start}..{end} step {step} do {body}")]
#[kirin(fn, type = T)]
pub struct For<T: CompileTimeValue + Default> {
    induction_var: SSAValue,
    start: SSAValue,
    end: SSAValue,
    step: SSAValue,
    body: Block,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

/// Terminates an SCF body block, yielding a value back to the parent
/// `If` or `For` operation. Analogous to MLIR's `scf.yield`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "yield {value}")]
#[kirin(terminator, type = T)]
pub struct Yield<T: CompileTimeValue + Default> {
    value: SSAValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
