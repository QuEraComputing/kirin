use std::convert::TryFrom;

use kirin::ir;
use kirin::prelude::*;
use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_interpreter_2::{
    BlockSeed, BranchCondition, ConsumeEffect, Cursor, Interpretable, InterpreterError, Lift,
    Machine, ProductValue, ValueStore, control::Directive, interpreter::SingleStage,
};

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

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
            _ => {
                InterpreterError::message_err("only i64 arith constants are supported in SCF tests")
            }
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

impl ForLoopValue for TestValue {
    fn loop_condition(&self, end: &Self) -> Option<bool> {
        match (self, end) {
            (TestValue::I64(a), TestValue::I64(b)) => Some(a < b),
            _ => None,
        }
    }

    fn loop_step(&self, step: &Self) -> Option<Self> {
        match (self, step) {
            (TestValue::I64(a), TestValue::I64(b)) => a.checked_add(*b).map(TestValue::I64),
            _ => None,
        }
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

impl Lift<TestEffect> for Cursor<Block> {
    fn lift(self) -> TestEffect {
        match self {
            Cursor::Advance => TestEffect::Advance,
            Cursor::Stay => TestEffect::Advance,
            Cursor::Jump(block) => panic!("unexpected Jump in SCF test: {block:?}"),
        }
    }
}

#[derive(Debug, Default)]
struct TestMachine;

impl<'ir> Machine<'ir> for TestMachine {
    type Effect = TestEffect;
    type Stop = TestValue;
    type Seed = BlockSeed<TestValue>;
}

impl<'ir> ConsumeEffect<'ir> for TestMachine {
    type Error = InterpreterError;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<Directive<Self::Stop, Self::Seed>, Self::Error> {
        Ok(match effect {
            TestEffect::Advance => Directive::Advance,
            TestEffect::Return(value) => Directive::Stop(value),
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

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for ScfTestLang {
    type Effect = TestEffect;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        match self {
            ScfTestLang::Scf(scf) => match scf {
                StructuredControlFlow::If(op) => op.interpret(interp).map(Lift::lift),
                StructuredControlFlow::For(op) => op.interpret(interp).map(Lift::lift),
                StructuredControlFlow::Yield(op) => {
                    // Delegate to the generic Yield impl which always errors
                    let result: Result<Cursor<Block>, InterpreterError> = op.interpret(interp);
                    result.map(Lift::lift)
                }
            },
            ScfTestLang::Constant(op) => {
                let value = TestValue::try_from(op.value.clone())?;
                interp.write(op.result, value)?;
                Ok(TestEffect::Advance)
            }
            ScfTestLang::FunctionBody(_) => InterpreterError::unsupported_err(
                "function bodies are structural and should not be stepped directly",
            ),
            ScfTestLang::Return(op) => {
                let values: Vec<_> = op.arguments().copied().collect();
                match values.as_slice() {
                    [value] => Ok(TestEffect::Return(interp.read(*value)?)),
                    [] => InterpreterError::unsupported_err("void return not supported in test"),
                    _ => InterpreterError::unsupported_err("multi-return not supported in test"),
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

// ---------------------------------------------------------------------------
// Helpers for building SCF For programs
// ---------------------------------------------------------------------------

/// Build: f(end) = result = for iv in 0..end step 1 iter_args(0) do { yield iv }; return result
///
/// Each iteration yields the induction variable. After the loop finishes,
/// the carried state equals the last iv value yielded (end - 1).
/// For end=5, result=4.
fn build_for_yield_iv_program(
    pipeline: &mut Pipeline<StageInfo<ScfTestLang>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        // Build entry block with 1 arg: end
        let entry_block = b.block().argument(ArithType::I64).new();
        let end_ssa: SSAValue = b.block_arena()[entry_block].arguments[0].into();

        // Build constants: start=0, step=1, init=0
        let const_start = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(0));
        let const_step = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let const_init = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(0));

        // Build body block with 2 args: [iv, carried]
        // Yield iv (body block's first argument)
        // We need to create the block first to get the block arg SSA values,
        // then create the yield referencing the iv arg.
        let body_block = b
            .block()
            .argument(ArithType::I64) // iv
            .argument(ArithType::I64) // carried
            .new();
        let iv_block_arg: SSAValue = b.block_arena()[body_block].arguments[0].into();

        // Create yield terminator that yields iv
        let yield_op: Yield<ArithType> = Yield {
            values: vec![iv_block_arg],
            marker: std::marker::PhantomData,
        };
        let yield_stmt = b
            .statement()
            .definition(ScfTestLang::Scf(StructuredControlFlow::Yield(yield_op)))
            .new();
        {
            use ir::query::ParentInfo;
            let block_info: &mut Item<BlockInfo<ScfTestLang>> =
                b.block_arena_mut().get_mut(body_block).unwrap();
            block_info.terminator = Some(yield_stmt);

            let stmt_info = &mut b.statement_arena_mut()[yield_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(body_block));
        }

        // Build the For statement
        let for_stmt_id = b.statement_arena().next_id();
        let for_result: ResultValue = b
            .ssa()
            .kind(ir::BuilderSSAKind::Result(for_stmt_id, 0))
            .ty(ArithType::I64)
            .new()
            .into();

        // induction_var is the SSA for the body block's first argument
        let for_op: For<ArithType> = For {
            induction_var: iv_block_arg,
            start: const_start.result.into(),
            end: end_ssa,
            step: const_step.result.into(),
            init_args: vec![const_init.result.into()],
            body: body_block,
            results: vec![for_result],
            marker: std::marker::PhantomData,
        };
        let for_stmt = b
            .statement()
            .definition(ScfTestLang::Scf(StructuredControlFlow::For(for_op)))
            .new();
        assert_eq!(for_stmt, for_stmt_id);

        // Build return
        let ret = Return::<ArithType>::new(b, vec![SSAValue::from(for_result)]);

        // Wire statements into entry block
        {
            use ir::query::ParentInfo;

            let const_start_stmt: Statement = const_start.into();
            let stmt_info = &mut b.statement_arena_mut()[const_start_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let const_step_stmt: Statement = const_step.into();
            let stmt_info = &mut b.statement_arena_mut()[const_step_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let const_init_stmt: Statement = const_init.into();
            let stmt_info = &mut b.statement_arena_mut()[const_init_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let stmt_info = &mut b.statement_arena_mut()[for_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let ret_stmt: Statement = ret.into();
            let stmt_info = &mut b.statement_arena_mut()[ret_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let linked =
                b.link_statements(&[const_start_stmt, const_step_stmt, const_init_stmt, for_stmt]);
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

/// Build: f() = result = for iv in 10..5 step 1 iter_args(42) do { yield iv }; return result
///
/// Start=10 >= end=5, so the loop body never executes.
/// Result should be the initial carried value: 42.
fn build_for_empty_range_program(
    pipeline: &mut Pipeline<StageInfo<ScfTestLang>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        // Build entry block with 0 args
        let entry_block = b.block().new();

        // Build constants: start=10, end=5, step=1, init=42
        let const_start = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(10));
        let const_end = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(5));
        let const_step = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let const_init = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(42));

        // Build body block with 2 args: [iv, carried]
        let body_block = b
            .block()
            .argument(ArithType::I64) // iv
            .argument(ArithType::I64) // carried
            .new();
        let iv_block_arg: SSAValue = b.block_arena()[body_block].arguments[0].into();

        // Create yield terminator that yields iv (won't actually execute)
        let yield_op: Yield<ArithType> = Yield {
            values: vec![iv_block_arg],
            marker: std::marker::PhantomData,
        };
        let yield_stmt = b
            .statement()
            .definition(ScfTestLang::Scf(StructuredControlFlow::Yield(yield_op)))
            .new();
        {
            use ir::query::ParentInfo;
            let block_info: &mut Item<BlockInfo<ScfTestLang>> =
                b.block_arena_mut().get_mut(body_block).unwrap();
            block_info.terminator = Some(yield_stmt);

            let stmt_info = &mut b.statement_arena_mut()[yield_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(body_block));
        }

        // Build the For statement
        let for_stmt_id = b.statement_arena().next_id();
        let for_result: ResultValue = b
            .ssa()
            .kind(ir::BuilderSSAKind::Result(for_stmt_id, 0))
            .ty(ArithType::I64)
            .new()
            .into();

        let for_op: For<ArithType> = For {
            induction_var: iv_block_arg,
            start: const_start.result.into(),
            end: const_end.result.into(),
            step: const_step.result.into(),
            init_args: vec![const_init.result.into()],
            body: body_block,
            results: vec![for_result],
            marker: std::marker::PhantomData,
        };
        let for_stmt = b
            .statement()
            .definition(ScfTestLang::Scf(StructuredControlFlow::For(for_op)))
            .new();
        assert_eq!(for_stmt, for_stmt_id);

        // Build return
        let ret = Return::<ArithType>::new(b, vec![SSAValue::from(for_result)]);

        // Wire statements into entry block
        {
            use ir::query::ParentInfo;

            let const_start_stmt: Statement = const_start.into();
            let stmt_info = &mut b.statement_arena_mut()[const_start_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let const_end_stmt: Statement = const_end.into();
            let stmt_info = &mut b.statement_arena_mut()[const_end_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let const_step_stmt: Statement = const_step.into();
            let stmt_info = &mut b.statement_arena_mut()[const_step_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let const_init_stmt: Statement = const_init.into();
            let stmt_info = &mut b.statement_arena_mut()[const_init_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let stmt_info = &mut b.statement_arena_mut()[for_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let ret_stmt: Statement = ret.into();
            let stmt_info = &mut b.statement_arena_mut()[ret_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let linked = b.link_statements(&[
                const_start_stmt,
                const_end_stmt,
                const_step_stmt,
                const_init_stmt,
                for_stmt,
            ]);
            let block_info = &mut b.block_arena_mut()[entry_block];
            block_info.statements = linked;
            block_info.terminator = Some(ret_stmt);
        }

        let region = b.region().add_block(entry_block).new();
        let body =
            FunctionBody::<ArithType>::new(b, region, Signature::new(vec![], ArithType::I64, ()));
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

/// Build: f() = for iv in 0..3 step 1 iter_args() do { yield }; return const 99
///
/// A void for loop (no carried state). The loop body yields nothing.
/// The loop runs 3 iterations then the function returns 99.
fn build_void_for_program(
    pipeline: &mut Pipeline<StageInfo<ScfTestLang>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        // Build entry block with 0 args
        let entry_block = b.block().new();

        // Build constants: start=0, end=3, step=1
        let const_start = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(0));
        let const_end = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(3));
        let const_step = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));

        // Build body block with 1 arg: [iv] (no carried values)
        let body_block = b
            .block()
            .argument(ArithType::I64) // iv
            .new();

        // Create yield terminator with no values
        let yield_op: Yield<ArithType> = Yield {
            values: vec![],
            marker: std::marker::PhantomData,
        };
        let yield_stmt = b
            .statement()
            .definition(ScfTestLang::Scf(StructuredControlFlow::Yield(yield_op)))
            .new();
        {
            use ir::query::ParentInfo;
            let block_info: &mut Item<BlockInfo<ScfTestLang>> =
                b.block_arena_mut().get_mut(body_block).unwrap();
            block_info.terminator = Some(yield_stmt);

            let stmt_info = &mut b.statement_arena_mut()[yield_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(body_block));
        }

        let iv_block_arg: SSAValue = b.block_arena()[body_block].arguments[0].into();

        // Build the For statement with 0 results
        let for_op: For<ArithType> = For {
            induction_var: iv_block_arg,
            start: const_start.result.into(),
            end: const_end.result.into(),
            step: const_step.result.into(),
            init_args: vec![],
            body: body_block,
            results: vec![],
            marker: std::marker::PhantomData,
        };
        let for_stmt = b
            .statement()
            .definition(ScfTestLang::Scf(StructuredControlFlow::For(for_op)))
            .new();

        // Build constant 99 to return
        let const_ret = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(99));

        // Build return
        let ret = Return::<ArithType>::new(b, vec![const_ret.result.into()]);

        // Wire statements into entry block
        {
            use ir::query::ParentInfo;

            let const_start_stmt: Statement = const_start.into();
            let stmt_info = &mut b.statement_arena_mut()[const_start_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let const_end_stmt: Statement = const_end.into();
            let stmt_info = &mut b.statement_arena_mut()[const_end_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let const_step_stmt: Statement = const_step.into();
            let stmt_info = &mut b.statement_arena_mut()[const_step_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let stmt_info = &mut b.statement_arena_mut()[for_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let const_ret_stmt: Statement = const_ret.into();
            let stmt_info = &mut b.statement_arena_mut()[const_ret_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let ret_stmt: Statement = ret.into();
            let stmt_info = &mut b.statement_arena_mut()[ret_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(entry_block));

            let linked = b.link_statements(&[
                const_start_stmt,
                const_end_stmt,
                const_step_stmt,
                for_stmt,
                const_ret_stmt,
            ]);
            let block_info = &mut b.block_arena_mut()[entry_block];
            block_info.statements = linked;
            block_info.terminator = Some(ret_stmt);
        }

        let region = b.region().add_block(entry_block).new();
        let body =
            FunctionBody::<ArithType>::new(b, region, Signature::new(vec![], ArithType::I64, ()));
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

// ---------------------------------------------------------------------------
// For tests
// ---------------------------------------------------------------------------

#[test]
fn for_yield_iv_accumulator() {
    // f(end=5) → for iv in 0..5 step 1 iter_args(0) do { yield iv } → result=4
    let mut pipeline: Pipeline<StageInfo<ScfTestLang>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_for_yield_iv_program(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    let result = interp.run_specialization(spec_fn, &[TestValue::I64(5)]);
    match result {
        Ok(kirin_interpreter_2::result::Run::Stopped(value)) => {
            assert_eq!(value, TestValue::I64(4));
        }
        other => panic!("expected Stopped(4), got: {other:?}"),
    }
}

#[test]
fn for_empty_range_returns_initial_carried() {
    // f() → for iv in 10..5 step 1 iter_args(42) do { yield iv } → result=42 (0 iterations)
    let mut pipeline: Pipeline<StageInfo<ScfTestLang>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_for_empty_range_program(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    let result = interp.run_specialization(spec_fn, &[]);
    match result {
        Ok(kirin_interpreter_2::result::Run::Stopped(value)) => {
            assert_eq!(value, TestValue::I64(42));
        }
        other => panic!("expected Stopped(42), got: {other:?}"),
    }
}

#[test]
fn void_for_runs_without_error() {
    // f() → for iv in 0..3 step 1 iter_args() do { yield }; return 99 → result=99
    let mut pipeline: Pipeline<StageInfo<ScfTestLang>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_void_for_program(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    let result = interp.run_specialization(spec_fn, &[]);
    match result {
        Ok(kirin_interpreter_2::result::Run::Stopped(value)) => {
            assert_eq!(value, TestValue::I64(99));
        }
        other => panic!("expected Stopped(99), got: {other:?}"),
    }
}
