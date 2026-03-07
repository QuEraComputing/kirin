//! Tests for interpreter error paths that are never triggered in the happy-path
//! test suite: MaxDepthExceeded, UnboundValue, ArityMismatch, Halt, MissingEntry.

use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::{FunctionBody, Return};
use kirin_interpreter::{
    ConcreteExt, Continuation, Frame, InterpreterError, StackInterpreter, StageAccess,
};
use kirin_ir::query::ParentInfo;
use kirin_ir::*;
use kirin_test_languages::CompositeLanguage;
use kirin_test_utils::ir_fixtures::first_statement_of_specialization;

// ===========================================================================
// Helper: a language with kirin_function::Call for recursive tests
// ===========================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[wraps]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
enum RecursiveLang {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    Call(kirin_function::Call<ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

/// Build a self-recursive function `f(x) = if x then f(x-1) else 0`.
fn build_recursive_func(
    pipeline: &mut Pipeline<StageInfo<RecursiveLang>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let func = pipeline.function().name("rec").new();
    let staged = pipeline
        .staged_function::<RecursiveLang>()
        .func(func)
        .stage(stage_id)
        .new()
        .unwrap();

    let stage = pipeline.stage_mut(stage_id).unwrap();

    // entry(x): c1 = const 1; dec = sub x, c1; cond_br x call_block(dec) exit_block()
    let entry = stage.block().argument(ArithType::I64).new();
    let call_block = stage.block().argument(ArithType::I64).new();
    let exit_block = stage.block().new();

    let x: SSAValue = entry.expect_info(stage).arguments[0].into();
    let call_arg: SSAValue = call_block.expect_info(stage).arguments[0].into();

    // exit_block: c0 = const 0; ret c0
    let c0 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(0));
    let ret0 = Return::<ArithType>::new(stage, c0.result);
    {
        let stmts: Vec<Statement> = vec![c0.into()];
        for stmt in &stmts {
            *stmt.expect_info_mut(stage).get_parent_mut() = Some(exit_block);
        }
        let linked = stage.link_statements(&stmts);
        let ret_stmt: Statement = ret0.into();
        *ret_stmt.expect_info_mut(stage).get_parent_mut() = Some(exit_block);
        let exit_info = exit_block.get_info_mut(stage).unwrap();
        exit_info.statements = linked;
        exit_info.terminator = Some(ret_stmt);
    }

    // call_block(arg): call rec(arg); ret call_result
    let rec_symbol = stage.symbol_table_mut().intern("rec".to_string());
    let call = kirin_function::Call::<ArithType>::new(stage, rec_symbol, vec![call_arg]);
    let ret_call = Return::<ArithType>::new(stage, call.res);
    {
        let call_stmt: Statement = call.into();
        *call_stmt.expect_info_mut(stage).get_parent_mut() = Some(call_block);
        let linked = stage.link_statements(&[call_stmt]);
        let ret_stmt: Statement = ret_call.into();
        *ret_stmt.expect_info_mut(stage).get_parent_mut() = Some(call_block);
        let call_info = call_block.get_info_mut(stage).unwrap();
        call_info.statements = linked;
        call_info.terminator = Some(ret_stmt);
    }

    // entry: c1 = const 1; dec = sub x, c1; cond_br x call_block(dec) exit_block()
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let dec = Arith::<ArithType>::op_sub(stage, x, c1.result);
    let cond = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        x,
        Successor::from_block(call_block),
        vec![dec.result.into()],
        Successor::from_block(exit_block),
        vec![],
    );
    {
        let stmts: Vec<Statement> = vec![c1.into(), dec.into()];
        for stmt in &stmts {
            *stmt.expect_info_mut(stage).get_parent_mut() = Some(entry);
        }
        let linked = stage.link_statements(&stmts);
        let cond_stmt: Statement = cond.into();
        *cond_stmt.expect_info_mut(stage).get_parent_mut() = Some(entry);
        let entry_info = entry.get_info_mut(stage).unwrap();
        entry_info.statements = linked;
        entry_info.terminator = Some(cond_stmt);
    }

    let region = stage
        .region()
        .add_block(entry)
        .add_block(call_block)
        .add_block(exit_block)
        .new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    stage
        .specialize()
        .staged_func(staged)
        .body(body)
        .new()
        .unwrap()
}

// ===========================================================================
// MaxDepthExceeded: recursive call that exceeds the depth limit
// ===========================================================================

#[test]
fn test_max_depth_exceeded() {
    let mut pipeline: Pipeline<StageInfo<RecursiveLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_recursive_func(&mut pipeline, stage_id);

    // Set max depth to 3 — rec(10) would need 10 frames, so it should fail.
    let mut interp: StackInterpreter<i64, _> =
        StackInterpreter::new(&pipeline, stage_id).with_max_depth(3);

    let result = interp.call(spec_fn, stage_id, &[10]);
    assert!(
        result.is_err(),
        "expected MaxDepthExceeded, got: {result:?}"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, InterpreterError::MaxDepthExceeded),
        "expected MaxDepthExceeded, got: {err:?}"
    );
}

#[test]
fn test_max_depth_exactly_sufficient() {
    let mut pipeline: Pipeline<StageInfo<RecursiveLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_recursive_func(&mut pipeline, stage_id);

    // rec(2) needs 3 frames: rec(2) -> rec(1) -> rec(0).
    // With max_depth=3 this should succeed.
    let mut interp: StackInterpreter<i64, _> =
        StackInterpreter::new(&pipeline, stage_id).with_max_depth(3);

    let result = interp.call(spec_fn, stage_id, &[2]).unwrap();
    assert_eq!(result, 0);
}

#[test]
fn test_max_depth_one_too_few() {
    let mut pipeline: Pipeline<StageInfo<RecursiveLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_recursive_func(&mut pipeline, stage_id);

    // rec(2) needs 3 frames but max_depth=2 — should fail.
    let mut interp: StackInterpreter<i64, _> =
        StackInterpreter::new(&pipeline, stage_id).with_max_depth(2);

    let err = interp.call(spec_fn, stage_id, &[2]).unwrap_err();
    assert!(
        matches!(err, InterpreterError::MaxDepthExceeded),
        "expected MaxDepthExceeded, got: {err:?}"
    );
}

// ===========================================================================
// UnboundValue: read a value that was never written
// ===========================================================================

#[test]
fn test_unbound_value_in_frame() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage = pipeline.stage_mut(stage_id).unwrap();

    // Build a minimal specialization just for the frame.
    let sf = stage.staged_function().new().unwrap();
    let c0 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(0));
    let ret = Return::<ArithType>::new(stage, c0.result);
    let block = stage.block().stmt(c0).terminator(ret).new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().staged_func(sf).body(body).new().unwrap();
    let first_stmt = first_statement_of_specialization(&pipeline, stage_id, spec_fn);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
    let frame = Frame::new(spec_fn, stage_id, first_stmt);
    interp.push_frame(frame).unwrap();

    // Create a bogus SSAValue that was never written.
    let bogus_ssa = SSAValue::from(TestSSAValue(9999));
    use kirin_interpreter::ValueStore;
    let err = interp.read(bogus_ssa).unwrap_err();
    assert!(
        matches!(err, InterpreterError::UnboundValue(v) if v == bogus_ssa),
        "expected UnboundValue({bogus_ssa:?}), got: {err:?}"
    );
}

// ===========================================================================
// ArityMismatch: wrong number of block arguments
// ===========================================================================

#[test]
fn test_arity_mismatch_too_few_args() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage = pipeline.stage_mut(stage_id).unwrap();

    // Build a block with 2 arguments.
    let sf = stage.staged_function().new().unwrap();
    let ba_x = stage.block_argument(0);
    let ret = Return::<ArithType>::new(stage, SSAValue::from(ba_x));
    let block = stage
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().staged_func(sf).body(body).new().unwrap();
    let first_stmt = first_statement_of_specialization(&pipeline, stage_id, spec_fn);

    let stage_info = pipeline.stage(stage_id).unwrap();
    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
    let frame = Frame::new(spec_fn, stage_id, first_stmt);
    interp.push_frame(frame).unwrap();

    // Bind only 1 arg to a block expecting 2.
    use kirin_interpreter::BlockEvaluator;
    let err = interp
        .bind_block_args::<CompositeLanguage>(stage_info, block, &[42_i64])
        .unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::ArityMismatch {
                expected: 2,
                got: 1
            }
        ),
        "expected ArityMismatch {{ expected: 2, got: 1 }}, got: {err:?}"
    );
}

#[test]
fn test_arity_mismatch_too_many_args() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage = pipeline.stage_mut(stage_id).unwrap();

    // Build a block with 1 argument.
    let sf = stage.staged_function().new().unwrap();
    let ba_x = stage.block_argument(0);
    let ret = Return::<ArithType>::new(stage, SSAValue::from(ba_x));
    let block = stage.block().argument(ArithType::I64).terminator(ret).new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().staged_func(sf).body(body).new().unwrap();
    let first_stmt = first_statement_of_specialization(&pipeline, stage_id, spec_fn);

    let stage_info = pipeline.stage(stage_id).unwrap();
    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
    let frame = Frame::new(spec_fn, stage_id, first_stmt);
    interp.push_frame(frame).unwrap();

    // Bind 3 args to a block expecting 1.
    use kirin_interpreter::BlockEvaluator;
    let err = interp
        .bind_block_args::<CompositeLanguage>(stage_info, block, &[1, 2, 3])
        .unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::ArityMismatch {
                expected: 1,
                got: 3
            }
        ),
        "expected ArityMismatch {{ expected: 1, got: 3 }}, got: {err:?}"
    );
}

#[test]
fn test_arity_mismatch_zero_args_block() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage = pipeline.stage_mut(stage_id).unwrap();

    // Build a block with 0 arguments.
    let sf = stage.staged_function().new().unwrap();
    let c0 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(0));
    let ret = Return::<ArithType>::new(stage, c0.result);
    let block = stage.block().stmt(c0).terminator(ret).new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().staged_func(sf).body(body).new().unwrap();
    let first_stmt = first_statement_of_specialization(&pipeline, stage_id, spec_fn);

    let stage_info = pipeline.stage(stage_id).unwrap();
    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
    let frame = Frame::new(spec_fn, stage_id, first_stmt);
    interp.push_frame(frame).unwrap();

    // Bind 2 args to a block expecting 0.
    use kirin_interpreter::BlockEvaluator;
    let err = interp
        .bind_block_args::<CompositeLanguage>(stage_info, block, &[1, 2])
        .unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::ArityMismatch {
                expected: 0,
                got: 2
            }
        ),
        "expected ArityMismatch {{ expected: 0, got: 2 }}, got: {err:?}"
    );
}

// ===========================================================================
// ArityMismatch through the call interface (wrong number of function args)
// ===========================================================================

#[test]
fn test_arity_mismatch_through_call() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    // build_add_one expects 1 arg.
    let spec_fn = kirin_test_utils::ir_fixtures::build_add_one(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // Pass 0 args — should get ArityMismatch.
    let err = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_fn, &[])
        .unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::ArityMismatch {
                expected: 1,
                got: 0
            }
        ),
        "expected ArityMismatch {{ expected: 1, got: 0 }}, got: {err:?}"
    );
}

// ===========================================================================
// Halt continuation during nested call
// ===========================================================================

/// A statement that always returns Halt when interpreted by StackInterpreter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
struct HaltStmt;

impl<'ir, I, L> kirin_interpreter::Interpretable<'ir, I, L> for HaltStmt
where
    I: kirin_interpreter::Interpreter<'ir, Ext = ConcreteExt>,
    I::Error: From<InterpreterError>,
    L: Dialect + 'ir,
{
    fn interpret(&self, _interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        Ok(Continuation::Ext(ConcreteExt::Halt))
    }
}

/// A language that includes a halt statement for testing Halt during nested calls.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, CallSemantics)]
#[wraps]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
enum HaltLang {
    Constant(Constant<ArithValue, ArithType>),
    Call(kirin_function::Call<ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
    Halt(HaltStmt),
}

/// Manual Interpretable impl for HaltLang — delegates to each inner type.
/// We need this because HaltStmt requires I::Ext = ConcreteExt.
impl<'ir, I, L> kirin_interpreter::Interpretable<'ir, I, L> for HaltLang
where
    I: kirin_interpreter::Interpreter<'ir, Ext = ConcreteExt>,
    I::Value: Clone
        + std::ops::Add<Output = I::Value>
        + std::ops::Sub<Output = I::Value>
        + std::ops::Mul<Output = I::Value>
        + std::ops::Neg<Output = I::Value>
        + std::ops::Div<Output = I::Value>
        + std::ops::Rem<Output = I::Value>
        + From<i64>
        + From<ArithValue>,
    I::Error: From<InterpreterError>,
    I::StageInfo: HasStageInfo<L>,
    L: Dialect + 'ir,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        match self {
            HaltLang::Constant(inner) => {
                <Constant<ArithValue, ArithType> as kirin_interpreter::Interpretable<'ir, I, L>>::interpret(inner, interp)
            }
            HaltLang::Call(inner) => {
                <kirin_function::Call<ArithType> as kirin_interpreter::Interpretable<'ir, I, L>>::interpret(inner, interp)
            }
            HaltLang::FunctionBody(inner) => {
                <FunctionBody<ArithType> as kirin_interpreter::Interpretable<'ir, I, L>>::interpret(inner, interp)
            }
            HaltLang::Return(inner) => {
                <Return<ArithType> as kirin_interpreter::Interpretable<'ir, I, L>>::interpret(inner, interp)
            }
            HaltLang::Halt(inner) => {
                <HaltStmt as kirin_interpreter::Interpretable<'ir, I, L>>::interpret(inner, interp)
            }
        }
    }
}

#[test]
fn test_halt_during_nested_call() {
    let mut pipeline: Pipeline<StageInfo<HaltLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    // Build callee: a function whose body contains a HaltStmt.
    let callee_func = pipeline.function().name("halter").new();
    let callee_staged = pipeline
        .staged_function::<HaltLang>()
        .func(callee_func)
        .stage(stage_id)
        .new()
        .unwrap();

    let stage = pipeline.stage_mut(stage_id).unwrap();
    let halt = HaltStmt::new(stage);
    let block = stage.block().stmt(halt).new();
    // No terminator — the Halt interrupts before we need one.
    let region = stage.region().add_block(block).new();
    let callee_body = FunctionBody::<ArithType>::new(stage, region);
    let _callee_spec = stage
        .specialize()
        .staged_func(callee_staged)
        .body(callee_body)
        .new()
        .unwrap();

    // Build caller: calls the callee function by name.
    let caller_func = pipeline.function().name("caller").new();
    let caller_staged = pipeline
        .staged_function::<HaltLang>()
        .func(caller_func)
        .stage(stage_id)
        .new()
        .unwrap();

    let caller_spec = {
        let stage = pipeline.stage_mut(stage_id).unwrap();
        let halter_sym = stage.symbol_table_mut().intern("halter".to_string());
        let call = kirin_function::Call::<ArithType>::new(stage, halter_sym, vec![]);
        let ret = Return::<ArithType>::new(stage, call.res);
        let block = stage.block().stmt(call).terminator(ret).new();
        let region = stage.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(stage, region);
        stage
            .specialize()
            .staged_func(caller_staged)
            .body(body)
            .new()
            .unwrap()
    };

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
    let err = interp.call(caller_spec, stage_id, &[]).unwrap_err();
    assert!(
        matches!(err, InterpreterError::UnexpectedControl(ref msg) if msg.contains("halt")),
        "expected UnexpectedControl(halt), got: {err:?}"
    );
}

// ===========================================================================
// MissingEntry: body returns non-Jump from interpret
// ===========================================================================

/// A callable body that always fails to resolve an entry block.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
struct BadBody {
    body: Region,
}

impl<'ir, I, L> kirin_interpreter::Interpretable<'ir, I, L> for BadBody
where
    I: kirin_interpreter::Interpreter<'ir>,
    I::Error: From<InterpreterError>,
    L: Dialect + 'ir,
{
    fn interpret(&self, _interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        // Body interpret is not used via SSACFGRegion path.
        Err(InterpreterError::MissingEntry.into())
    }
}

impl kirin_interpreter::SSACFGRegion for BadBody {
    fn entry_block<L: Dialect>(&self, _stage: &StageInfo<L>) -> Result<Block, InterpreterError> {
        Err(InterpreterError::MissingEntry)
    }
}

/// A language with BadBody as the callable — entry_block always fails.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[wraps]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
enum BadBodyLang {
    Constant(Constant<ArithValue, ArithType>),
    #[callable]
    BadBody(BadBody),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

#[test]
fn test_missing_entry_from_bad_body() {
    let mut pipeline: Pipeline<StageInfo<BadBodyLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let block = stage.block().new();
    let region = stage.region().add_block(block).new();
    let bad_body = BadBody::new(stage, region);
    let spec_fn = stage
        .specialize()
        .staged_func(sf)
        .body(bad_body)
        .new()
        .unwrap();

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
    let err = interp
        .in_stage::<BadBodyLang>()
        .call(spec_fn, &[])
        .unwrap_err();
    assert!(
        matches!(err, InterpreterError::MissingEntry),
        "expected MissingEntry, got: {err:?}"
    );
}
