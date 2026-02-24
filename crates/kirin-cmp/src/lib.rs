#[cfg(feature = "interpret")]
mod interpret_impl;
#[cfg(feature = "interpret")]
pub use interpret_impl::CompareValue;

use kirin::prelude::*;

/// Generic comparison statements parameterized by a compile-time type lattice.
///
/// Each operation takes two operands and produces a result of the same type `T`.
/// The result convention follows integer semantics: 1 for true, 0 for false.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(pure, fn, type = T)]
pub enum Cmp<T: CompileTimeValue + Default> {
    #[kirin(speculatable)]
    #[chumsky(format = "{result:name} = eq {lhs}, {rhs} -> {result:type}")]
    Eq {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "{result:name} = ne {lhs}, {rhs} -> {result:type}")]
    Ne {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "{result:name} = lt {lhs}, {rhs} -> {result:type}")]
    Lt {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "{result:name} = le {lhs}, {rhs} -> {result:type}")]
    Le {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "{result:name} = gt {lhs}, {rhs} -> {result:type}")]
    Gt {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "{result:name} = ge {lhs}, {rhs} -> {result:type}")]
    Ge {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
}
