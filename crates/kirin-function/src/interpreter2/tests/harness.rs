use std::{
    convert::Infallible,
    convert::TryFrom,
    ops::{Add, Mul, Neg, Sub},
    rc::Rc,
};

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::{Constant, interpreter2 as constant2};
use kirin_interpreter::BranchCondition;
use kirin_interpreter_2::{
    ConsumeEffect, Interpretable, Interpreter, InterpreterError, LiftEffect, LiftStop,
    ProductValue, ProjectMachine, ProjectMachineMut, control::Shell, interpreter::SingleStage,
};

use crate::{Bind, Call, FunctionBody, Return};

use crate::interpreter2::{Effect, Machine as FunctionMachine};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestEffect {
    Leaf(constant2::Effect),
    Function(Effect<TestValue>),
}

#[derive(Debug)]
pub struct TestMachine {
    function: FunctionMachine<TestValue>,
}

impl TestMachine {
    pub fn frame_depth(&self) -> usize {
        self.function.frame_depth()
    }
}

impl Default for TestMachine {
    fn default() -> Self {
        Self {
            function: FunctionMachine::new(),
        }
    }
}

impl<'ir> kirin_interpreter_2::Machine<'ir> for TestMachine {
    type Effect = TestEffect;
    type Stop = TestValue;
}

impl<'ir> ConsumeEffect<'ir> for TestMachine {
    type Error = InterpreterError;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Shell<Self::Stop>, Self::Error> {
        match effect {
            TestEffect::Leaf(effect) => Ok(effect.map_stop(|never| match never {}).into_shell()),
            TestEffect::Function(effect) => self.function.consume_effect(effect),
        }
    }
}

impl ProjectMachine<FunctionMachine<TestValue>> for TestMachine {
    fn project(&self) -> &FunctionMachine<TestValue> {
        &self.function
    }
}

impl ProjectMachineMut<FunctionMachine<TestValue>> for TestMachine {
    fn project_mut(&mut self) -> &mut FunctionMachine<TestValue> {
        &mut self.function
    }
}

impl<'ir> LiftEffect<'ir, FunctionMachine<TestValue>> for TestMachine {
    fn lift_effect(effect: Effect<TestValue>) -> TestEffect {
        TestEffect::Function(effect)
    }
}

impl<'ir> LiftEffect<'ir, kirin_interpreter_2::effect::Stateless<Infallible>> for TestMachine {
    fn lift_effect(effect: kirin_interpreter_2::effect::Flow<Infallible>) -> TestEffect {
        TestEffect::Leaf(effect)
    }
}

impl<'ir> LiftStop<'ir, FunctionMachine<TestValue>> for TestMachine {
    fn lift_stop(stop: TestValue) -> TestValue {
        stop
    }
}

pub type TestInterp<'ir> = SingleStage<'ir, TestLanguage, TestValue, TestMachine, InterpreterError>;

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for TestLanguage {
    type Machine = TestMachine;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        match self {
            TestLanguage::Constant(op) => interp.interpret_lifted(op),
            TestLanguage::Arith(op) => interp.interpret_lifted(op),
            TestLanguage::ControlFlow(op) => interp.interpret_lifted(op),
            TestLanguage::FunctionBody(op) => interp.interpret_lifted(op),
            TestLanguage::Bind(op) => interp.interpret_lifted(op),
            TestLanguage::Call(op) => interp.interpret_lifted(op),
            TestLanguage::Return(op) => interp.interpret_lifted(op),
        }
    }
}
