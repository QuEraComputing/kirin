#![allow(dead_code)]

use std::convert::Infallible;

use kirin_interpreter_3::{
    BlockSeed, BranchCondition, Effect, Execute, InterpError, Interpretable, ProductValue,
    SingleStage, ValueRead,
};
use kirin_ir::{Block, Dialect, Placeholder, Region, ResultValue, SSAValue, Signature, Successor};
use smallvec::SmallVec;

use super::{TestMachine, TestType, TestValue};

type TestInterpreter<'ir> = SingleStage<'ir, TestDialect, TestValue, TestMachine>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct FunctionDef {
    pub body: Region,
    pub sig: Signature<TestType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct ConstI64 {
    pub value: i64,
    pub result: ResultValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct AddI64 {
    pub lhs: SSAValue,
    pub rhs: SSAValue,
    pub result: ResultValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, terminator, type = TestType, crate = kirin_ir)]
pub struct JumpTo {
    pub target: Successor,
    pub args: Vec<SSAValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, terminator, type = TestType, crate = kirin_ir)]
pub struct StopOp {
    pub value: SSAValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, terminator, type = TestType, crate = kirin_ir)]
pub struct YieldOp {
    pub value: SSAValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, terminator, type = TestType, crate = kirin_ir)]
pub struct ReturnOp {
    pub value: SSAValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct PackTuple {
    pub lhs: SSAValue,
    pub rhs: SSAValue,
    pub result: ResultValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct UnknownValue {
    pub result: ResultValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, terminator, type = TestType, crate = kirin_ir)]
pub struct BranchSelect {
    pub condition: SSAValue,
    pub true_target: Successor,
    pub true_args: Vec<SSAValue>,
    pub false_target: Successor,
    pub false_args: Vec<SSAValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct IfOp {
    pub condition: SSAValue,
    pub then_body: Block,
    pub else_body: Block,
    pub result: ResultValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub struct ForOp {
    pub start: SSAValue,
    pub end: SSAValue,
    pub step: SSAValue,
    pub init: SSAValue,
    pub body: Block,
    pub result: ResultValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = TestType, crate = kirin_ir)]
pub enum TestDialect {
    #[wraps]
    FunctionDef(FunctionDef),
    #[wraps]
    ConstI64(ConstI64),
    #[wraps]
    AddI64(AddI64),
    #[wraps]
    JumpTo(JumpTo),
    #[wraps]
    StopOp(StopOp),
    #[wraps]
    YieldOp(YieldOp),
    #[wraps]
    ReturnOp(ReturnOp),
    #[wraps]
    PackTuple(PackTuple),
    #[wraps]
    UnknownValue(UnknownValue),
    #[wraps]
    BranchSelect(BranchSelect),
    #[wraps]
    IfOp(IfOp),
    #[wraps]
    ForOp(ForOp),
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for ConstI64 {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        _interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        Ok(
            Effect::BindValue(self.result.into(), TestValue::from(self.value))
                .then(Effect::Advance),
        )
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for AddI64 {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        let lhs = match interp.read(self.lhs)? {
            TestValue::I64(value) => value,
            other => panic!("expected i64 lhs, got {other:?}"),
        };
        let rhs = match interp.read(self.rhs)? {
            TestValue::I64(value) => value,
            other => panic!("expected i64 rhs, got {other:?}"),
        };

        Ok(Effect::BindValue(self.result.into(), TestValue::from(lhs + rhs)).then(Effect::Advance))
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for JumpTo {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        let args: SmallVec<[TestValue; 2]> = self
            .args
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        Ok(Effect::Jump(self.target.target(), args))
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for StopOp {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        Ok(Effect::Stop(interp.read(self.value)?))
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for YieldOp {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        Ok(Effect::Yield(interp.read(self.value)?))
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for ReturnOp {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        Ok(Effect::Return(interp.read(self.value)?))
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for PackTuple {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        let lhs = interp.read(self.lhs)?;
        let rhs = interp.read(self.rhs)?;
        Ok(
            Effect::BindValue(self.result.into(), TestValue::new_product(vec![lhs, rhs]))
                .then(Effect::Advance),
        )
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for UnknownValue {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        _interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        Ok(Effect::BindValue(self.result.into(), TestValue::Unknown).then(Effect::Advance))
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for BranchSelect {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        let condition = interp.read(self.condition)?;
        let (target, args) = match condition.is_truthy() {
            Some(true) => (self.true_target.target(), &self.true_args),
            Some(false) => (self.false_target.target(), &self.false_args),
            None => {
                return Err(kirin_interpreter_3::InterpreterError::unsupported(
                    "nondeterministic branch conditions are not supported in interpreter-3",
                )
                .into());
            }
        };

        let args: SmallVec<[TestValue; 2]> = args
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        Ok(Effect::Jump(target, args))
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for IfOp {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        let condition = interp.read(self.condition)?;
        let block = match condition.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => {
                return Err(kirin_interpreter_3::InterpreterError::unsupported(
                    "nondeterministic scf.if",
                )
                .into());
            }
        };

        match BlockSeed::entry(block).execute(interp)? {
            Effect::Yield(value) => {
                Ok(Effect::BindValue(self.result.into(), value).then(Effect::Advance))
            }
            _ => Err(kirin_interpreter_3::InterpreterError::unsupported(
                "expected yield from scf.if body",
            )
            .into()),
        }
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for ForOp {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        let mut iv = expect_i64(interp.read(self.start)?)?;
        let end = expect_i64(interp.read(self.end)?)?;
        let step = expect_i64(interp.read(self.step)?)?;
        let mut carried = interp.read(self.init)?;

        while iv <= end {
            match BlockSeed::new(
                self.body,
                SmallVec::from_vec(vec![TestValue::from(iv), carried]),
            )
            .execute(interp)?
            {
                Effect::Yield(value) => carried = value,
                _ => {
                    return Err(kirin_interpreter_3::InterpreterError::unsupported(
                        "expected yield from scf.for body",
                    )
                    .into());
                }
            }

            iv = iv.checked_add(step).ok_or_else(|| {
                InterpError::from(kirin_interpreter_3::InterpreterError::unsupported(
                    "induction variable overflow",
                ))
            })?;
        }

        Ok(Effect::BindValue(self.result.into(), carried).then(Effect::Advance))
    }
}

fn expect_i64(value: TestValue) -> Result<i64, InterpError<Infallible>> {
    match value {
        TestValue::I64(value) => Ok(value),
        TestValue::Bool(_) | TestValue::Unknown | TestValue::Tuple(_) => Err(
            kirin_interpreter_3::InterpreterError::unsupported("expected i64 test value").into(),
        ),
    }
}

impl<'ir> Interpretable<TestInterpreter<'ir>> for TestDialect {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        interp: &mut TestInterpreter<'ir>,
    ) -> Result<Effect<TestValue, Self::Effect>, InterpError<Self::Error>> {
        match self {
            Self::FunctionDef(_) => unreachable!("function bodies are not directly stepped"),
            Self::ConstI64(inner) => inner.interpret(interp),
            Self::AddI64(inner) => inner.interpret(interp),
            Self::JumpTo(inner) => inner.interpret(interp),
            Self::StopOp(inner) => inner.interpret(interp),
            Self::YieldOp(inner) => inner.interpret(interp),
            Self::ReturnOp(inner) => inner.interpret(interp),
            Self::PackTuple(inner) => inner.interpret(interp),
            Self::UnknownValue(inner) => inner.interpret(interp),
            Self::BranchSelect(inner) => inner.interpret(interp),
            Self::IfOp(inner) => inner.interpret(interp),
            Self::ForOp(inner) => inner.interpret(interp),
        }
    }
}
