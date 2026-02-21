#![allow(dead_code)]

use std::fmt;

use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::FunctionBody;
use kirin_interpreter::{
    BranchCondition, CallSemantics, Continuation, Interpretable, Interpreter, InterpreterError,
};
use kirin_ir::*;
use kirin_test_utils::{Interval, interval_add, interval_mul, interval_neg, interval_sub};

// ---------------------------------------------------------------------------
// Combined dialect usable by both concrete and abstract tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[wraps]
#[kirin(fn, type = ArithType, crate = "kirin_ir")]
pub enum TestDialect {
    Arith(Arith<ArithType>),
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
}

// ---------------------------------------------------------------------------
// ArithmeticValue: shared trait for both concrete (i64) and abstract (Interval)
// ---------------------------------------------------------------------------

pub trait ArithmeticValue: Clone + Sized {
    fn arith_add(&self, other: &Self) -> Self;
    fn arith_sub(&self, other: &Self) -> Self;
    fn arith_mul(&self, other: &Self) -> Self;
    fn arith_div(&self, other: &Self) -> Option<Self>;
    fn arith_rem(&self, other: &Self) -> Option<Self>;
    fn arith_neg(&self) -> Self;
    fn from_arith_value(v: &ArithValue) -> Self;
}

impl ArithmeticValue for i64 {
    fn arith_add(&self, other: &Self) -> Self {
        self + other
    }
    fn arith_sub(&self, other: &Self) -> Self {
        self - other
    }
    fn arith_mul(&self, other: &Self) -> Self {
        self * other
    }
    fn arith_div(&self, other: &Self) -> Option<Self> {
        if *other == 0 {
            None
        } else {
            Some(self / other)
        }
    }
    fn arith_rem(&self, other: &Self) -> Option<Self> {
        if *other == 0 {
            None
        } else {
            Some(self % other)
        }
    }
    fn arith_neg(&self) -> Self {
        -self
    }
    fn from_arith_value(v: &ArithValue) -> Self {
        match v {
            ArithValue::I64(x) => *x,
            ArithValue::I32(x) => *x as i64,
            ArithValue::I16(x) => *x as i64,
            ArithValue::I8(x) => *x as i64,
            _ => panic!("unsupported ArithValue for i64: {v:?}"),
        }
    }
}

impl ArithmeticValue for Interval {
    fn arith_add(&self, other: &Self) -> Self {
        interval_add(self, other)
    }
    fn arith_sub(&self, other: &Self) -> Self {
        interval_sub(self, other)
    }
    fn arith_mul(&self, other: &Self) -> Self {
        interval_mul(self, other)
    }
    fn arith_div(&self, _other: &Self) -> Option<Self> {
        // Simplified: just return top for division
        Some(Interval::top())
    }
    fn arith_rem(&self, _other: &Self) -> Option<Self> {
        Some(Interval::top())
    }
    fn arith_neg(&self) -> Self {
        interval_neg(self)
    }
    fn from_arith_value(v: &ArithValue) -> Self {
        match v {
            ArithValue::I64(x) => Interval::constant(*x),
            ArithValue::I32(x) => Interval::constant(*x as i64),
            ArithValue::I16(x) => Interval::constant(*x as i64),
            ArithValue::I8(x) => Interval::constant(*x as i64),
            _ => Interval::top(),
        }
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct DivisionByZero;

impl fmt::Display for DivisionByZero {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "division by zero")
    }
}

impl std::error::Error for DivisionByZero {}

// ---------------------------------------------------------------------------
// Generic Interpretable impl
// ---------------------------------------------------------------------------

impl<I> CallSemantics<I, TestDialect> for TestDialect
where
    I: Interpreter<Error = InterpreterError>,
    I::StageInfo: HasStageInfo<TestDialect>,
    I::Value: ArithmeticValue + BranchCondition,
    FunctionBody<ArithType>: CallSemantics<I, TestDialect>,
{
    type Result = <FunctionBody<ArithType> as CallSemantics<I, TestDialect>>::Result;

    fn call_semantics(
        &self,
        interp: &mut I,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<Self::Result, InterpreterError> {
        match self {
            TestDialect::FunctionBody(body) => body.call_semantics(interp, callee, args),
            _ => Err(InterpreterError::MissingEntry),
        }
    }
}

impl<I> Interpretable<I, Self> for TestDialect
where
    I: Interpreter<Error = InterpreterError>,
    I::StageInfo: HasStageInfo<Self>,
    I::Value: ArithmeticValue + BranchCondition,
{
    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Continuation<I::Value, I::Ext>, InterpreterError> {
        match self {
            TestDialect::Arith(arith) => match arith {
                Arith::Add {
                    lhs, rhs, result, ..
                } => {
                    let a = interp.read(*lhs)?;
                    let b = interp.read(*rhs)?;
                    interp.write(*result, a.arith_add(&b))?;
                    Ok(Continuation::Continue)
                }
                Arith::Sub {
                    lhs, rhs, result, ..
                } => {
                    let a = interp.read(*lhs)?;
                    let b = interp.read(*rhs)?;
                    interp.write(*result, a.arith_sub(&b))?;
                    Ok(Continuation::Continue)
                }
                Arith::Mul {
                    lhs, rhs, result, ..
                } => {
                    let a = interp.read(*lhs)?;
                    let b = interp.read(*rhs)?;
                    interp.write(*result, a.arith_mul(&b))?;
                    Ok(Continuation::Continue)
                }
                Arith::Div {
                    lhs, rhs, result, ..
                } => {
                    let a = interp.read(*lhs)?;
                    let b = interp.read(*rhs)?;
                    let v = a
                        .arith_div(&b)
                        .ok_or_else(|| InterpreterError::custom(DivisionByZero))?;
                    interp.write(*result, v)?;
                    Ok(Continuation::Continue)
                }
                Arith::Rem {
                    lhs, rhs, result, ..
                } => {
                    let a = interp.read(*lhs)?;
                    let b = interp.read(*rhs)?;
                    let v = a
                        .arith_rem(&b)
                        .ok_or_else(|| InterpreterError::custom(DivisionByZero))?;
                    interp.write(*result, v)?;
                    Ok(Continuation::Continue)
                }
                Arith::Neg {
                    operand, result, ..
                } => {
                    let a = interp.read(*operand)?;
                    interp.write(*result, a.arith_neg())?;
                    Ok(Continuation::Continue)
                }
            },

            TestDialect::ControlFlow(cf) => {
                <ControlFlow<ArithType> as Interpretable<I, Self>>::interpret(cf, interp)
            }

            TestDialect::Constant(c) => {
                let val = I::Value::from_arith_value(&c.value);
                interp.write(c.result, val)?;
                Ok(Continuation::Continue)
            }

            TestDialect::FunctionBody(body) => {
                <FunctionBody<ArithType> as Interpretable<I, Self>>::interpret(body, interp)
            }
        }
    }
}
