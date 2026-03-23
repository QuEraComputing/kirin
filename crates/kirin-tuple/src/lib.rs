//! Tuple pack/unpack dialect for Kirin.
//!
//! This dialect provides language-level tuple operations that complement
//! the IR multi-result mechanism. It bridges two distinct levels of
//! abstraction:
//!
//! - **IR multi-result**: an operation produces N separate SSA values, each
//!   with its own type (`Continuation::Yield(SmallVec<[V; 1]>)`). This is a
//!   dataflow concept.
//! - **Language-level tuple**: a single SSA value of a product type that
//!   contains multiple values. This is a type system concept.
//!
//! A language can use IR multi-result, language-level tuples via this dialect,
//! or both. They compose cleanly: `NewTuple` packs SSA values into a single
//! tuple value, `Unpack` destructures a tuple back into multiple SSA values.
//!
//! # Statements
//!
//! | Statement | Description |
//! |-----------|-------------|
//! | `new_tuple(%a, %b, ..) -> T` | Pack multiple SSA values into a single tuple value |
//! | `unpack %t -> T, T, ..` | Destructure a tuple value into multiple SSA values (multi-result) |
//!
//! # Design Context: Why a Tuple Dialect?
//!
//! MLIR defines a builtin `tuple<T1, T2>` type but provides **no standard
//! operations** to construct or destructure it. The type was added in MLIR's
//! early days as a "shell type, without semantics" to bridge non-MLIR
//! representations that used tuples for multi-result. Once MLIR operations
//! gained native multi-result support, the tuple type became largely
//! redundant. Sean Silva (MLIR core) called having a builtin type with no
//! operations "an anti-pattern." The community consensus is that dialects
//! should define their own tuple-like types rather than rely on the builtin.
//!
//! In practice, multiple MLIR downstreams (CIRCT, Flang, CIR) each
//! independently implement the same three operations: pack, unpack, and
//! element access. This dialect standardizes that pattern for Kirin.
//!
//! Kirin's approach differs from MLIR in three ways:
//!
//! 1. The tuple type is **not builtin** — it lives in the user's value enum,
//!    following the MLIR community's own recommendation for dialect-specific
//!    types.
//! 2. The operations are **standardized in a composable dialect** — avoiding
//!    the "every downstream reinvents the same ops" problem.
//! 3. The [`TupleValue`] trait lets each language define its own packing
//!    semantics — matching the "shell type, without semantics" intent, but
//!    with actual operations to work with.
//!
//! References:
//! - [Rationale for not having tuple type operations](https://discourse.llvm.org/t/rationale-for-not-having-tuple-type-operations-in-the-main-dialects/3748)
//! - [Rationale behind MLIR's builtin tuple type](https://discourse.llvm.org/t/rationale-behind-mlirs-builtin-tuple-type/84424)
//!
//! # Extension Point
//!
//! Dialect authors implement [`TupleValue`] on their value types to define
//! how tuple packing/unpacking works at the interpreter level.

mod interpret_impl;
pub use interpret_impl::TupleValue;

use kirin::prelude::*;

#[cfg(test)]
mod tests;

/// Wrapper enum that composes all tuple operations.
///
/// Use `#[wraps]` delegation so that each variant's `Dialect` impl is
/// forwarded to the inner type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[wraps]
#[kirin(builders, type = T)]
pub enum Tuple<T: CompileTimeValue> {
    NewTuple(NewTuple<T>),
    Unpack(Unpack<T>),
}

/// Packs multiple SSA values into a single tuple value.
///
/// The result is a single `ResultValue` holding the packed tuple.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$new_tuple({args}) -> {result:type}")]
#[kirin(builders, type = T)]
pub struct NewTuple<T: CompileTimeValue> {
    args: Vec<SSAValue>,
    result: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

/// Destructures a tuple value into multiple SSA values (multi-result).
///
/// Uses `Vec<ResultValue>` to support an arbitrary number of output values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$unpack {source} -> {results:type}")]
#[kirin(builders, type = T)]
pub struct Unpack<T: CompileTimeValue> {
    source: SSAValue,
    results: Vec<ResultValue>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
