mod interpret_impl;
pub use interpret_impl::CompareValue;

use kirin::prelude::*;

#[cfg(test)]
mod tests;

/// Generic comparison statements parameterized by a compile-time type lattice.
///
/// Each operation takes two operands and produces a result of the same type `T`.
/// The result convention follows integer semantics: 1 for true, 0 for false.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[non_exhaustive]
#[kirin(pure, builders, type = T)]
pub enum Cmp<T: CompileTimeValue> {
    #[kirin(speculatable)]
    #[chumsky(format = "$eq {lhs}, {rhs} -> {result:type}")]
    Eq {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "$ne {lhs}, {rhs} -> {result:type}")]
    Ne {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "$lt {lhs}, {rhs} -> {result:type}")]
    Lt {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "$le {lhs}, {rhs} -> {result:type}")]
    Le {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "$gt {lhs}, {rhs} -> {result:type}")]
    Gt {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "$ge {lhs}, {rhs} -> {result:type}")]
    Ge {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
    },
    #[doc(hidden)]
    __Phantom(std::marker::PhantomData<T>),
}
