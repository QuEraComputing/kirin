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
//! | `if %cond then {..} else {..} [-> types]` | Two-way conditional with 0-to-N results |
//! | `for %iv in %lo..%hi step %s iter_args(..) do {..} [-> types]` | Counted loop with multi-accumulator support |
//! | `yield [%v1, %v2, ..]` | Terminates an SCF body block, yielding 0-to-N values to the parent |
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

mod value;
pub use value::ForLoopValue;
pub mod interpreter;

use kirin::prelude::*;
use kirin_interpreter::Interpretable;

#[cfg(test)]
mod tests;

/// Wrapper enum that composes all structured control flow operations.
///
/// Use `#[wraps]` delegation so that each variant's `Dialect` impl is
/// forwarded to the inner type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint, Interpretable)]
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
/// Supports void-if (0 results) through multi-result (N results).
/// Corresponds to MLIR's `scf.if`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$if {condition} then {then_body} else {else_body}[ -> {results:type}]")]
#[kirin(builders, type = T)]
pub struct If<T: CompileTimeValue> {
    condition: SSAValue,
    then_body: Block,
    else_body: Block,
    results: Vec<ResultValue>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> If<T> {
    /// The SSA value driving the branch decision.
    pub fn condition(&self) -> SSAValue {
        self.condition
    }

    /// The `then` arm body (a single block terminated by `yield`).
    pub fn then_block(&self) -> Block {
        self.then_body
    }

    /// The `else` arm body (a single block terminated by `yield`).
    pub fn else_block(&self) -> Block {
        self.else_body
    }

    /// The result slots produced by the conditional (one per yielded value).
    pub fn results(&self) -> &[ResultValue] {
        &self.results
    }
}

/// Counted loop with an induction variable ranging from `start` to `end`
/// (exclusive) with a given `step`. The `body` block receives the current
/// induction variable as a block argument.
///
/// Supports multi-accumulator loops with `Vec<ResultValue>` results.
/// Corresponds to MLIR's `scf.for`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(
    format = "$for {induction_var} in {start}..{end} step {step} iter_args({init_args}) do {body}[ -> {results:type}]"
)]
#[kirin(builders, type = T)]
pub struct For<T: CompileTimeValue> {
    induction_var: SSAValue,
    start: SSAValue,
    end: SSAValue,
    step: SSAValue,
    init_args: Vec<SSAValue>,
    body: Block,
    results: Vec<ResultValue>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> For<T> {
    /// The induction variable (also the first block argument of `body`).
    pub fn induction_var(&self) -> SSAValue {
        self.induction_var
    }

    /// Lower bound of the induction range.
    pub fn start(&self) -> SSAValue {
        self.start
    }

    /// Upper bound (exclusive) of the induction range.
    pub fn end(&self) -> SSAValue {
        self.end
    }

    /// Induction step.
    pub fn step(&self) -> SSAValue {
        self.step
    }

    /// Initial loop-carried values, positionally matched to the body's
    /// carried block arguments (`body` args after the induction variable),
    /// the body's `yield` values, and `results`.
    pub fn init_args(&self) -> &[SSAValue] {
        &self.init_args
    }

    /// The loop body (a single block terminated by `yield`). Its first block
    /// argument is the induction variable; the rest are loop-carried.
    pub fn body(&self) -> Block {
        self.body
    }

    /// The result slots produced by the loop (final loop-carried values).
    pub fn results(&self) -> &[ResultValue] {
        &self.results
    }
}

/// Terminates an SCF body block, yielding values back to the parent
/// `If` or `For` operation. Analogous to MLIR's `scf.yield`.
///
/// Supports zero values (void-if) through N values (multi-result).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$yield[ {values}]")]
#[kirin(terminator, type = T)]
pub struct Yield<T: CompileTimeValue> {
    values: Vec<SSAValue>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> Yield<T> {
    /// The values yielded back to the parent `If`/`For`, positionally matched
    /// to the parent's result slots (and, for a loop, the next iteration's
    /// loop-carried block arguments).
    pub fn values(&self) -> &[SSAValue] {
        &self.values
    }
}
