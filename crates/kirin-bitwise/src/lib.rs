//! Generic bitwise and shift dialect for Kirin.
//!
//! # Usage
//!
//! Compose this dialect into your language by wrapping `Bitwise<T>` with your
//! language's type lattice:
//!
//! ```rust,ignore
//! use kirin::ir::Dialect;
//! use kirin_arith::ArithType;
//! use kirin_bitwise::Bitwise;
//! use kirin_cf::ControlFlow;
//!
//! #[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
//! #[wraps]
//! #[kirin(fn, type = ArithType)]
//! enum IntegerLanguage {
//!     Bitwise(Bitwise<ArithType>),
//!     ControlFlow(ControlFlow<ArithType>),
//! }
//! ```
//!
//! Bitwise statements use a uniform text format where result type drives
//! semantics:
//!
//! ```text
//! %r = and %a, %b -> i32
//! %r = or %a, %b -> u64
//! %r = xor %a, %b -> i8
//! %r = not %a -> i16
//! %r = shl %a, %b -> u32
//! %r = shr %a, %b -> i32
//! ```
//!
//! # Semantics
//!
//! - `and`, `or`, `xor`, `not` are pure and speculatable.
//! - `shl` and `shr` are pure.
//! - `shr` has one operation form; signedness of the operand/result type
//!   determines arithmetic vs logical shift semantics.
//! - Verifier passes are expected to enforce type compatibility, including the
//!   RFC rule that shift count type must match the shifted value type.

use kirin::prelude::*;

/// Generic bitwise statements parameterized by a compile-time type lattice.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(pure, fn, type = T)]
pub enum Bitwise<T: CompileTimeValue + Default> {
    #[kirin(speculatable)]
    #[chumsky(format = "{result:name} = and {lhs}, {rhs} -> {result:type}")]
    And {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "{result:name} = or {lhs}, {rhs} -> {result:type}")]
    Or {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "{result:name} = xor {lhs}, {rhs} -> {result:type}")]
    Xor {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[kirin(speculatable)]
    #[chumsky(format = "{result:name} = not {operand} -> {result:type}")]
    Not {
        operand: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = shl {lhs}, {rhs} -> {result:type}")]
    Shl {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = shr {lhs}, {rhs} -> {result:type}")]
    Shr {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
}
