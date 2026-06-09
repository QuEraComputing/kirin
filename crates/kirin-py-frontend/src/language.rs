//! The target Kirin language for lowered Python: a composition of existing
//! dialects, mirroring `example/toy-lang`'s `HighLevel`.

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::{Call, Function, Lexical, Return};
use kirin_interpreter::{FunctionEntry, Interpretable};
use kirin_scf::{For, If, StructuredControlFlow, Yield};

/// Single-stage language used by the Python front-end: lexical functions +
/// structured control flow + constants + arithmetic + comparison.
///
/// `Interpretable` + `FunctionEntry` make the lowered IR *executable* (the inner
/// dialects already provide their interpreter impls), so tests can run a kernel
/// and check the computed result, not just that the IR is well-formed.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Dialect, FunctionEntry, HasParser, PrettyPrint, Interpretable,
)]
#[kirin(builders, type = ArithType)]
pub enum PyLang {
    #[wraps]
    #[callable]
    Lexical(Lexical<ArithType>),
    #[wraps]
    Structured(StructuredControlFlow<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cmp(Cmp<ArithType>),
}

/// A pipeline whose single stage holds `PyLang` IR.
pub type PyPipeline = Pipeline<StageInfo<PyLang>>;

// `#[wraps]` derives `From` for the wrapped dialects (Lexical, StructuredControlFlow,
// Constant, Arith, Cmp). The inner-dialect builders we call (`Function::new`,
// `If::new`, ...) require `From` impls for the *inner* statement types too, so we
// chain through the wrapper dialect.
impl From<Function<ArithType>> for PyLang {
    fn from(v: Function<ArithType>) -> Self {
        Self::from(Lexical::from(v))
    }
}

impl From<Call<ArithType>> for PyLang {
    fn from(v: Call<ArithType>) -> Self {
        Self::from(Lexical::from(v))
    }
}

impl From<Return<ArithType>> for PyLang {
    fn from(v: Return<ArithType>) -> Self {
        Self::from(Lexical::from(v))
    }
}

impl From<If<ArithType>> for PyLang {
    fn from(v: If<ArithType>) -> Self {
        Self::from(StructuredControlFlow::from(v))
    }
}

impl From<For<ArithType>> for PyLang {
    fn from(v: For<ArithType>) -> Self {
        Self::from(StructuredControlFlow::from(v))
    }
}

impl From<Yield<ArithType>> for PyLang {
    fn from(v: Yield<ArithType>) -> Self {
        Self::from(StructuredControlFlow::from(v))
    }
}
