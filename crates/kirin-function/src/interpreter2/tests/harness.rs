use std::{
    convert::TryFrom,
    ops::{Add, Mul, Neg, Sub},
    rc::Rc,
};

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_2::{
    ConsumeEffect, Interpretable, InterpreterError, Machine, ProductValue, control::Shell,
    effect::Cursor, interpreter::SingleStage,
};

use crate::{Bind, Call, FunctionBody, Return};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin::ir)]
#[wraps]
pub enum TestLanguage {
    Constant(Constant<ArithValue, ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Arith(Arith<ArithType>),
    FunctionBody(FunctionBody<ArithType>),
    Bind(Bind<ArithType>),
    Call(Call<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestValueKind {
    I64(i64),
    Product(Product<TestValue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestValue(Rc<TestValueKind>);

pub fn i64(value: i64) -> TestValue {
    TestValue(Rc::new(TestValueKind::I64(value)))
}

impl ProductValue for TestValue {
    fn as_product(&self) -> Option<&Product<Self>> {
        match self.0.as_ref() {
            TestValueKind::Product(product) => Some(product),
            TestValueKind::I64(_) => None,
        }
    }

    fn from_product(product: Product<Self>) -> Self {
        Self(Rc::new(TestValueKind::Product(product)))
    }
}

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

fn expect_i64(value: &TestValue) -> Result<i64, InterpreterError> {
    match value.0.as_ref() {
        TestValueKind::I64(value) => Ok(*value),
        TestValueKind::Product(_) => Err(unsupported("expected scalar i64 value")),
    }
}

impl TryFrom<ArithValue> for TestValue {
    type Error = InterpreterError;

    fn try_from(value: ArithValue) -> Result<Self, Self::Error> {
        match value {
            ArithValue::I64(value) => Ok(i64(value)),
            _ => Err(unsupported(
                "only i64 arith constants are supported in interpreter2 tests",
            )),
        }
    }
}

impl Add for TestValue {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        i64(expect_i64(&self).unwrap() + expect_i64(&rhs).unwrap())
    }
}

impl Sub for TestValue {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        i64(expect_i64(&self).unwrap() - expect_i64(&rhs).unwrap())
    }
}

impl Mul for TestValue {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        i64(expect_i64(&self).unwrap() * expect_i64(&rhs).unwrap())
    }
}

impl Neg for TestValue {
    type Output = Self;

    fn neg(self) -> Self::Output {
        i64(-expect_i64(&self).unwrap())
    }
}

impl kirin_arith::CheckedDiv for TestValue {
    fn checked_div(self, rhs: Self) -> Option<Self> {
        expect_i64(&self)
            .ok()?
            .checked_div(expect_i64(&rhs).ok()?)
            .map(i64)
    }
}

impl kirin_arith::CheckedRem for TestValue {
    fn checked_rem(self, rhs: Self) -> Option<Self> {
        expect_i64(&self)
            .ok()?
            .checked_rem(expect_i64(&rhs).ok()?)
            .map(i64)
    }
}

impl BranchCondition for TestValue {
    fn is_truthy(&self) -> Option<bool> {
        expect_i64(self).ok().map(|value| value != 0)
    }
}

/// Effect type for the function test machine.
///
/// Uses Shell<TestValue, Block> so that both cursor-like effects (Advance, Stay)
/// and stopping effects (Stop) can be represented.
type TestEffect = Shell<TestValue, Block>;

/// Machine for function tests that supports both cursor and stop effects.
#[derive(Debug, Default)]
pub struct TestMachine;

impl<'ir> Machine<'ir> for TestMachine {
    type Effect = TestEffect;
    type Stop = TestValue;
    type Seed = Block;
}

impl<'ir> ConsumeEffect<'ir> for TestMachine {
    type Error = InterpreterError;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<Shell<Self::Stop, Self::Seed>, Self::Error> {
        Ok(effect)
    }
}

pub type TestInterp<'ir> = SingleStage<'ir, TestLanguage, TestValue, TestMachine, InterpreterError>;

/// Lift a unit-seed cursor into the test effect type.
fn lift_cursor(cursor: Cursor) -> TestEffect {
    match cursor {
        Cursor::Advance => Shell::Advance,
        Cursor::Stay => Shell::Stay,
        Cursor::Jump(()) => unreachable!("unit-seed cursor should never Jump"),
    }
}

/// Lift a block-seed cursor into the test effect type.
fn lift_block_cursor(cursor: Cursor<Block>) -> TestEffect {
    match cursor {
        Cursor::Advance => Shell::Advance,
        Cursor::Stay => Shell::Stay,
        Cursor::Jump(block) => Shell::Replace(block),
    }
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for TestLanguage {
    type Effect = TestEffect;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        match self {
            TestLanguage::Constant(op) => op.interpret(interp).map(lift_cursor),
            TestLanguage::Arith(op) => op.interpret(interp).map(lift_cursor),
            TestLanguage::ControlFlow(op) => op.interpret(interp).map(lift_block_cursor),
            TestLanguage::FunctionBody(op) => op.interpret(interp),
            TestLanguage::Bind(op) => op.interpret(interp),
            TestLanguage::Call(op) => op.interpret(interp),
            TestLanguage::Return(op) => op.interpret(interp),
        }
    }
}
