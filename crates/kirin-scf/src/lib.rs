//! Structured control flow dialect for Kirin.
//!
//! This dialect provides high-level control flow operations that model
//! structured programming constructs. Unlike `kirin-cf` which uses
//! unstructured branches, `kirin-scf` operations have lexically scoped
//! regions with guaranteed single-entry semantics.
//!
//! # Operations
//!
//! | Operation | Description |
//! |-----------|-------------|
//! | `if %cond then {..} else {..}` | Two-way conditional with single-block bodies |
//! | `for %iv in %lo..%hi step %s do {..}` | Counted loop with induction variable |
//! | `yield %v` | Terminates an SCF body block, yielding a value to the parent |
//!
//! # Block vs Region
//!
//! All body fields use `Block` (not `Region`) because MLIR's `scf.if` and
//! `scf.for` have the `SingleBlock` + `SingleBlockImplicitTerminator<YieldOp>`
//! traits. A `yield` terminates each body block.
//!
//! # MLIR Correspondence
//!
//! - `If` ↔ `scf.if`
//! - `For` ↔ `scf.for`
//! - `Yield` ↔ `scf.yield`

mod interpret_impl;
pub use interpret_impl::ForLoopValue;

use kirin::prelude::*;

#[cfg(test)]
mod tests;

/// Wrapper enum that composes all structured control flow operations.
///
/// Use `#[wraps]` delegation so that each variant's `Dialect` impl is
/// forwarded to the inner type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[wraps]
#[kirin(builders, type = T)]
pub enum StructuredControlFlow<T: CompileTimeValue> {
    If(If<T>),
    For(For<T>),
    #[kirin(terminator)]
    Yield(Yield<T>),
}

/// Two-way conditional: evaluates `then_body` or `else_body` depending
/// on `condition`. Both bodies are single blocks terminated by `yield`.
///
/// Corresponds to MLIR's `scf.if`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$if {condition} then {then_body} else {else_body} -> {result:type}")]
#[kirin(builders, type = T)]
pub struct If<T: CompileTimeValue> {
    condition: SSAValue,
    then_body: Block,
    else_body: Block,
    result: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

/// Counted loop with an induction variable ranging from `start` to `end`
/// (exclusive) with a given `step`. The `body` block receives the current
/// induction variable as a block argument.
///
/// Corresponds to MLIR's `scf.for`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(
    format = "$for {induction_var} in {start}..{end} step {step} iter_args({init_args}) do {body} -> {result:type}"
)]
#[kirin(builders, type = T)]
pub struct For<T: CompileTimeValue> {
    induction_var: SSAValue,
    start: SSAValue,
    end: SSAValue,
    step: SSAValue,
    init_args: Vec<SSAValue>,
    body: Block,
    result: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

/// Terminates an SCF body block, yielding a value back to the parent
/// `If` or `For` operation. Analogous to MLIR's `scf.yield`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$yield {value}")]
#[kirin(terminator, type = T)]
pub struct Yield<T: CompileTimeValue> {
    value: SSAValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
