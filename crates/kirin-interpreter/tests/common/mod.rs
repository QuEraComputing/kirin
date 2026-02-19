use std::fmt;

use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::FunctionBody;
use kirin_interpreter::{
    BranchCondition, InterpretControl, Interpretable, Interpreter, InterpreterError,
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
    fn is_negative(&self) -> bool;
    fn is_non_negative(&self) -> bool;
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
    fn is_negative(&self) -> bool {
        *self < 0
    }
    fn is_non_negative(&self) -> bool {
        *self >= 0
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
    fn is_negative(&self) -> bool {
        // Conservative: only negative if entire interval is below 0
        match self.hi {
            kirin_test_utils::Bound::NegInf => true,
            kirin_test_utils::Bound::Finite(h) => h < 0,
            kirin_test_utils::Bound::PosInf => false,
        }
    }
    fn is_non_negative(&self) -> bool {
        // Conservative: only non-negative if entire interval is >= 0
        match self.lo {
            kirin_test_utils::Bound::Finite(l) => l >= 0,
            kirin_test_utils::Bound::PosInf => true,
            kirin_test_utils::Bound::NegInf => false,
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

impl<I> Interpretable<I> for TestDialect
where
    I: Interpreter<Error = InterpreterError>,
    I::Value: ArithmeticValue + BranchCondition,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Control, InterpreterError> {
        match self {
            TestDialect::Arith(arith) => match arith {
                Arith::Add {
                    lhs, rhs, result, ..
                } => {
                    let a = interp.read(*lhs)?;
                    let b = interp.read(*rhs)?;
                    interp.write(*result, a.arith_add(&b))?;
                    Ok(I::Control::ctrl_continue())
                }
                Arith::Sub {
                    lhs, rhs, result, ..
                } => {
                    let a = interp.read(*lhs)?;
                    let b = interp.read(*rhs)?;
                    interp.write(*result, a.arith_sub(&b))?;
                    Ok(I::Control::ctrl_continue())
                }
                Arith::Mul {
                    lhs, rhs, result, ..
                } => {
                    let a = interp.read(*lhs)?;
                    let b = interp.read(*rhs)?;
                    interp.write(*result, a.arith_mul(&b))?;
                    Ok(I::Control::ctrl_continue())
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
                    Ok(I::Control::ctrl_continue())
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
                    Ok(I::Control::ctrl_continue())
                }
                Arith::Neg {
                    operand, result, ..
                } => {
                    let a = interp.read(*operand)?;
                    interp.write(*result, a.arith_neg())?;
                    Ok(I::Control::ctrl_continue())
                }
            },

            TestDialect::ControlFlow(cf) => cf.interpret(interp),

            TestDialect::Constant(c) => {
                let val = I::Value::from_arith_value(&c.value);
                interp.write(c.result, val)?;
                Ok(I::Control::ctrl_continue())
            }

            TestDialect::FunctionBody(_) => Ok(I::Control::ctrl_continue()),
        }
    }
}
