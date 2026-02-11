//! Generic arithmetic dialect for Kirin.
//!
//! # Usage
//!
//! Compose this dialect into your language by wrapping `Arith<T>` with your
//! language's type lattice:
//!
//! ```rust,ignore
//! use kirin::ir::Dialect;
//! use kirin_arith::{Arith, ArithType};
//! use kirin_cf::ControlFlow;
//!
//! #[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
//! #[wraps]
//! #[kirin(fn, type = ArithType)]
//! enum NumericLanguage {
//!     Arith(Arith<ArithType>),
//!     ControlFlow(ControlFlow<ArithType>),
//! }
//! ```
//!
//! Arithmetic statements use a uniform text format where result type drives
//! semantics:
//!
//! ```text
//! %sum = add %a, %b -> i32
//! %diff = sub %a, %b -> i64
//! %prod = mul %x, %y -> f64
//! %quot = div %x, %y -> f32
//! %rem = rem %a, %b -> i32
//! %neg = neg %a -> i32
//! ```
//!
//! # Design Principles
//!
//! - Reuse one operation set across languages (`add/sub/mul/div/rem/neg`).
//! - Keep operations generic over type lattice `T`.
//! - Keep IR-level arithmetic semantics abstract; lowering decides concrete behavior.
//!
//! `ArithType` and `ArithValue` are a batteries-included default for Rust-like
//! numeric types. If your language has different numeric semantics (for example,
//! big integers, decimal-first arithmetic, or domain-specific units), prefer
//! defining your own type/value pair and instantiate `Arith<YourType>`.

mod types;

use kirin::prelude::*;

pub use types::{ArithType, ArithValue};

/// Generic arithmetic statements parameterized by a compile-time type lattice.
///
/// This dialect is intentionally small and composable. It models core arithmetic
/// operations while leaving detailed runtime behavior to lowering/codegen.
///
/// # Usage
///
/// ```rust,ignore
/// use kirin::ir::{SSAKind, StageInfo};
/// use kirin_arith::{Arith, ArithType};
///
/// let mut stage: StageInfo<Arith<ArithType>> = StageInfo::default();
/// let a = stage
///     .ssa()
///     .name("a".to_string())
///     .ty(ArithType::I32)
///     .kind(SSAKind::Test)
///     .new();
/// let b = stage
///     .ssa()
///     .name("b".to_string())
///     .ty(ArithType::I32)
///     .kind(SSAKind::Test)
///     .new();
///
/// let add_stmt = Arith::<ArithType>::op_add(&mut stage, a, b);
/// let _sum = add_stmt.result;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(pure, fn, type = T)]
pub enum Arith<T: CompileTimeValue + Default> {
    #[chumsky(format = "{result:name} = add {lhs}, {rhs} -> {result:type}")]
    Add {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = sub {lhs}, {rhs} -> {result:type}")]
    Sub {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = mul {lhs}, {rhs} -> {result:type}")]
    Mul {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = div {lhs}, {rhs} -> {result:type}")]
    Div {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = rem {lhs}, {rhs} -> {result:type}")]
    Rem {
        lhs: SSAValue,
        rhs: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
    #[chumsky(format = "{result:name} = neg {operand} -> {result:type}")]
    Neg {
        operand: SSAValue,
        result: ResultValue,
        #[kirin(default)]
        marker: std::marker::PhantomData<T>,
    },
}
