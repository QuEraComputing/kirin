use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::{Call, Return};
use kirin_test_languages::SimpleType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint, Serialize, Deserialize)]
#[kirin(builders, type = SimpleType)]
pub enum BenchmarkLanguage {
    #[chumsky(format = "fn {:name}{signature} {body}")]
    Function {
        #[kirin(callable_body)]
        body: CFG,
        signature: Signature<SimpleType>,
    },
    #[wraps]
    Arith(Arith<SimpleType>),
    #[wraps]
    Cf(ControlFlow<SimpleType>),
    #[wraps]
    Return(Return<SimpleType>),
    #[wraps]
    Constant(Constant<Int, SimpleType>),
    #[wraps]
    Cmp(Cmp<SimpleType>),
    #[wraps]
    Call(Call<SimpleType>),
}

/// An `i64` literal. A local type so it can convert into both value domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Int(pub i64);

impl std::fmt::Display for Int {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Typeof<SimpleType> for Int {
    fn type_of(&self) -> SimpleType {
        SimpleType::I64
    }
}

impl kirin_prettyless::PrettyPrintViaDisplay for Int {}

impl kirin_chumsky::DirectlyParsable for Int {}

impl<'t> kirin_chumsky::HasParser<'t> for Int {
    type Output = Int;

    fn parser<I: kirin_chumsky::TokenInput<'t>>() -> kirin_chumsky::BoxedParser<'t, I, Int> {
        use kirin_chumsky::chumsky::Parser;
        <i64 as kirin_chumsky::HasParser>::parser().map(Int).boxed()
    }
}

impl From<Int> for i64 {
    fn from(value: Int) -> Self {
        value.0
    }
}

impl From<Int> for kirin_constprop::ConstPropValue<i64> {
    fn from(value: Int) -> Self {
        Self::Const(value.0)
    }
}

use kirin_interpreter::{ForwardEval, Interpretable, SparseForwardEffect, SparseForwardInterp};

impl<I> Interpretable<I, ForwardEval> for BenchmarkLanguage
where
    I: SparseForwardInterp,
    Arith<SimpleType>: Interpretable<I, ForwardEval>,
    ControlFlow<SimpleType>: Interpretable<I, ForwardEval>,
    Return<SimpleType>: Interpretable<I, ForwardEval>,
    Constant<Int, SimpleType>: Interpretable<I, ForwardEval>,
    Cmp<SimpleType>: Interpretable<I, ForwardEval>,
    Call<SimpleType>: Interpretable<I, ForwardEval>,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        match self {
            Self::Function { .. } => Ok(SparseForwardEffect::Next),
            Self::Arith(op) => op.interpret(interp),
            Self::Cf(op) => op.interpret(interp),
            Self::Return(op) => op.interpret(interp),
            Self::Constant(op) => op.interpret(interp),
            Self::Cmp(op) => op.interpret(interp),
            Self::Call(op) => op.interpret(interp),
        }
    }
}
