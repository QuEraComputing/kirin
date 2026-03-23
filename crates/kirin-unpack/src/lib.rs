//! Tuple pack/unpack dialect for Kirin.
//!
//! This dialect provides language-level tuple operations that complement
//! the IR multi-result mechanism. A language can use IR multi-result,
//! language-level tuples via `kirin-unpack`, or both.
//!
//! # Operations
//!
//! | Operation | Description |
//! |-----------|-------------|
//! | `make_tuple(%a, %b, ..) -> T` | Pack multiple SSA values into a single tuple value |
//! | `unpack %t -> T, T, ..` | Destructure a tuple value into multiple SSA values (multi-result) |
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
pub enum TupleOp<T: CompileTimeValue> {
    MakeTuple(MakeTuple<T>),
    Unpack(Unpack<T>),
}

/// Packs multiple SSA values into a single tuple value.
///
/// The result is a single `ResultValue` holding the packed tuple.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$make_tuple({args}) -> {result:type}")]
#[kirin(builders, type = T)]
pub struct MakeTuple<T: CompileTimeValue> {
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
