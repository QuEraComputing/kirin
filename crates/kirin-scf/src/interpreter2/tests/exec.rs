use std::convert::TryFrom;

use kirin::ir;
use kirin::prelude::*;
use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_interpreter::BranchCondition;
use kirin_interpreter_2::{
    ConsumeEffect, Cursor, Interpretable, InterpreterError, Lift, Machine, ProductValue,
    ValueStore, control::Shell, interpreter::SingleStage,
};

use crate::{If, StructuredControlFlow, Yield};

// ---------------------------------------------------------------------------
// Test value — wraps i64 with BranchCondition + ProductValue support
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestValue {
    I64(i64),
    Product(Box<ir::Product<TestValue>>),
}

impl TryFrom<ArithValue> for TestValue {
    type Error = InterpreterError;

    fn try_from(value: ArithValue) -> Result<Self, Self::Error> {
        match value {
            ArithValue::I64(v) => Ok(TestValue::I64(v)),
            _ => Err(InterpreterError::custom(std::io::Error::other(
                "only i64 arith constants are supported in SCF tests",
            ))),
        }
    }
}

impl BranchCondition for TestValue {
    fn is_truthy(&self) -> Option<bool> {
        match self {
            TestValue::I64(v) => Some(*v != 0),
            TestValue::Product(_) => None,
        }
    }
}

impl ProductValue for TestValue {
    fn as_product(&self) -> Option<&ir::Product<Self>> {
        match self {
            TestValue::Product(p) => Some(p),
            _ => None,
        }
    }

    fn from_product(product: ir::Product<Self>) -> Self {
        TestValue::Product(Box::new(product))
    }
}

// ---------------------------------------------------------------------------
// Test effect & machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestEffect {
    Advance,
    Return(TestValue),
}

impl Lift<TestEffect> for Cursor {
    fn lift(self) -> TestEffect {
        match self {
            Cursor::Advance => TestEffect::Advance,
            Cursor::Stay => TestEffect::Advance,
            Cursor::Jump(seed) => panic!("unexpected Jump in SCF test: {seed:?}"),
        }
    }
}

#[derive(Debug, Default)]
struct TestMachine;

impl<'ir> Machine<'ir> for TestMachine {
    type Effect = TestEffect;
    type Stop = TestValue;
}

impl<'ir> ConsumeEffect<'ir> for TestMachine {
    type Error = InterpreterError;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Shell<Self::Stop>, Self::Error> {
        Ok(match effect {
            TestEffect::Advance => Shell::Advance,
            TestEffect::Return(value) => Shell::Stop(value),
        })
    }
}

// ---------------------------------------------------------------------------
// Test language — wraps SCF + supporting ops for building programs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[wraps]
#[kirin(builders, type = ArithType)]
enum ScfTestLang {
    Scf(StructuredControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

type TestInterp<'ir> = SingleStage<'ir, ScfTestLang, TestValue, TestMachine, InterpreterError>;

// ---------------------------------------------------------------------------
// Interpretable impl — single dispatch for the whole language
// ---------------------------------------------------------------------------

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for ScfTestLang {
    type Effect = TestEffect;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        match self {
            ScfTestLang::Scf(scf) => match scf {
                StructuredControlFlow::If(op) => op.interpret(interp).map(Lift::lift),
                StructuredControlFlow::For(_) => {
                    Err(unsupported("scf.for not yet implemented in test harness"))
                }
                StructuredControlFlow::Yield(op) => {
                    // Delegate to the generic Yield impl which always errors
                    let result: Result<Cursor, InterpreterError> = op.interpret(interp);
                    result.map(Lift::lift)
                }
            },
            ScfTestLang::Constant(op) => {
                let value = TestValue::try_from(op.value.clone())?;
                interp.write(op.result, value)?;
                Ok(TestEffect::Advance)
            }
            ScfTestLang::FunctionBody(_) => Err(unsupported(
                "function bodies are structural and should not be stepped directly",
            )),
            ScfTestLang::Return(op) => {
                let values: Vec<_> = op.arguments().copied().collect();
                match values.as_slice() {
                    [value] => Ok(TestEffect::Return(interp.read(*value)?)),
                    [] => Err(unsupported("void return not supported in test")),
                    _ => Err(unsupported("multi-return not supported in test")),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — Yield outside SCF
// ---------------------------------------------------------------------------

/// Build a program where the entry block's terminator is a bare `yield`
/// (outside any scf.if or scf.for). Running it should error.
fn build_bare_yield_program(
    pipeline: &mut Pipeline<StageInfo<ScfTestLang>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        // Create a bare yield (no values, outside any scf body)
        let yield_op: Yield<ArithType> = Yield {
            values: vec![],
            marker: std::marker::PhantomData,
        };
        let yield_stmt = b
            .statement()
            .definition(ScfTestLang::Scf(StructuredControlFlow::Yield(yield_op)))
            .new();

        let block = b.block().new();
        {
            use ir::query::ParentInfo;
            let block_info: &mut Item<BlockInfo<ScfTestLang>> =
                b.block_arena_mut().get_mut(block).unwrap();
            block_info.terminator = Some(yield_stmt);

            let stmt_info = &mut b.statement_arena_mut()[yield_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(block));
        }

        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

#[test]
fn yield_outside_scf_errors() {
    let mut pipeline: Pipeline<StageInfo<ScfTestLang>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_bare_yield_program(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    let result = interp.run_specialization(spec_fn, &[]);

    assert!(result.is_err(), "yield outside scf.if/scf.for should error");
}

// ---------------------------------------------------------------------------
// Helpers for building SCF If programs
// ---------------------------------------------------------------------------

/// Build a yield-only block. Returns the block and its Yield statement's SSA
/// value references are set to `yield_values`.
///
/// The block has no arguments and no non-terminator statements — only a Yield
/// terminator that yields the given SSA values.
fn build_yield_block(
    b: &mut BuilderStageInfo<ScfTestLang>,
    yield_values: Vec<SSAValue>,
) -> ir::Block {
    let yield_op: Yield<ArithType> = Yield {
        values: yield_values,
        marker: std::marker::PhantomData,
    };
    let yield_stmt = b
        .statement()
        .definition(ScfTestLang::Scf(StructuredControlFlow::Yield(yield_op)))
        .new();

    let block = b.block().new();
    {
        use ir::query::ParentInfo;
        let block_info: &mut Item<BlockInfo<ScfTestLang>> =
            b.block_arena_mut().get_mut(block).unwrap();
        block_info.terminator = Some(yield_stmt);

        let stmt_info = &mut b.statement_arena_mut()[yield_stmt];
        *stmt_info.get_parent_mut() = Some(StatementParent::Block(block));
    }
    block
}

/// Build: f(cond, x) = result = if cond then { yield x } else { yield x }; return result
fn build_if_passthrough_program(
    pipeline: &mut Pipeline<StageInfo<ScfTestLang>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        // Build entry block first (with 2 args: cond, x) to get resolved SSA values
        let entry_block = b
            .block()
            .argument(ArithType::I64)
            .argument(ArithType::I64)
            .new();
        let cond_ssa: SSAValue = b.block_arena()[entry_block].arguments[0].into();
        let x_ssa: SSAValue = b.block_arena()[entry_block].arguments[1].into();

        // Build then_block: yield x
        let then_block = build_yield_block(b, vec![x_ssa]);

        // Build else_block: yield x
        let else_block = build_yield_block(b, vec![x_ssa]);

        // Build the If statement with one result
        let if_stmt_id = b.statement_arena().next_id();
        let if_result: ResultValue = b
            .ssa()
            .kind(ir::BuilderSSAKind::Result(if_stmt_id, 0))
            .ty(ArithType::I64)
            .new()
            .into();
        let if_op: If<ArithType> = If {
            condition: cond_ssa,
            then_body: then_block,
            else_body: else_block,
            results: vec![if_result],
            marker: std::marker::PhantomData,
        };
        let if_stmt = b
            .statement()
            .definition(ScfTestLang::Scf(StructuredControlFlow::If(if_op)))
            .new();
        assert_eq!(if_stmt, if_stmt_id);

        // Build the Return statement that returns the If result
        let ret = Return::<ArithType>::new(b, vec![SSAValue::from(if_result)]);

        // Wire statements into entry block
        {
            use ir::query::ParentInfo;

            // Add if_stmt to entry block's statement list
            let stmt_info = &mut b.statement_arena_mut()[if_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let ret_stmt: Statement = ret.into();
            let stmt_info = &mut b.statement_arena_mut()[ret_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let linked = b.link_statements(&[if_stmt]);
            let block_info = &mut b.block_arena_mut()[entry_block];
            block_info.statements = linked;
            block_info.terminator = Some(ret_stmt);
        }

        let region = b.region().add_block(entry_block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![ArithType::I64, ArithType::I64], ArithType::I64, ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

/// Build: f(cond) = if cond then { yield } else { yield }; return const 99
fn build_void_if_program(
    pipeline: &mut Pipeline<StageInfo<ScfTestLang>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        // Build entry block first (with 1 arg: cond)
        let entry_block = b.block().argument(ArithType::I64).new();
        let cond_ssa: SSAValue = b.block_arena()[entry_block].arguments[0].into();

        // Build then_block: yield (no values)
        let then_block = build_yield_block(b, vec![]);

        // Build else_block: yield (no values)
        let else_block = build_yield_block(b, vec![]);

        // Build the If statement with zero results
        let if_op: If<ArithType> = If {
            condition: cond_ssa,
            then_body: then_block,
            else_body: else_block,
            results: vec![],
            marker: std::marker::PhantomData,
        };
        let if_stmt = b
            .statement()
            .definition(ScfTestLang::Scf(StructuredControlFlow::If(if_op)))
            .new();

        // Build a constant 99 to return
        let const_op = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(99));

        // Build return
        let ret = Return::<ArithType>::new(b, vec![const_op.result.into()]);

        // Wire statements into entry block
        {
            use ir::query::ParentInfo;

            let stmt_info = &mut b.statement_arena_mut()[if_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let const_stmt: Statement = const_op.into();
            let stmt_info = &mut b.statement_arena_mut()[const_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let ret_stmt: Statement = ret.into();
            let stmt_info = &mut b.statement_arena_mut()[ret_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let linked = b.link_statements(&[if_stmt, const_stmt]);
            let block_info = &mut b.block_arena_mut()[entry_block];
            block_info.statements = linked;
            block_info.terminator = Some(ret_stmt);
        }

        let region = b.region().add_block(entry_block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![ArithType::I64], ArithType::I64, ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

// ---------------------------------------------------------------------------
// If tests
// ---------------------------------------------------------------------------

#[test]
fn if_true_branch_yields_value() {
    let mut pipeline: Pipeline<StageInfo<ScfTestLang>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_if_passthrough_program(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    // cond=1 (truthy), x=42
    let result = interp.run_specialization(spec_fn, &[TestValue::I64(1), TestValue::I64(42)]);
    match result {
        Ok(kirin_interpreter_2::result::Run::Stopped(value)) => {
            assert_eq!(value, TestValue::I64(42));
        }
        other => panic!("expected Stopped(42), got: {other:?}"),
    }
}

#[test]
fn if_false_branch_yields_value() {
    let mut pipeline: Pipeline<StageInfo<ScfTestLang>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_if_passthrough_program(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    // cond=0 (falsy), x=42
    let result = interp.run_specialization(spec_fn, &[TestValue::I64(0), TestValue::I64(42)]);
    match result {
        Ok(kirin_interpreter_2::result::Run::Stopped(value)) => {
            assert_eq!(value, TestValue::I64(42));
        }
        other => panic!("expected Stopped(42), got: {other:?}"),
    }
}

#[test]
fn void_if_runs_without_error() {
    let mut pipeline: Pipeline<StageInfo<ScfTestLang>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_void_if_program(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    // cond=1 (truthy) — takes then branch, which yields nothing
    let result = interp.run_specialization(spec_fn, &[TestValue::I64(1)]);
    match result {
        Ok(kirin_interpreter_2::result::Run::Stopped(value)) => {
            assert_eq!(value, TestValue::I64(99));
        }
        other => panic!("expected Stopped(99), got: {other:?}"),
    }
}
