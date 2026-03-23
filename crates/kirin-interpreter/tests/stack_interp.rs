//! StackInterpreter tests: concrete execution, fuel, breakpoints, frame push/pop,
//! session-style abstract interpretation with Interval, and multi-result writeback.

use kirin_interpreter::ProductValue;
use rustc_hash::FxHashSet;
use smallvec::smallvec;

use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_interpreter::{
    ConcreteExt, Continuation, Frame, InterpreterError, StackInterpreter, StageAccess,
};
use kirin_interval::Interval;
use kirin_ir::*;
use kirin_test_languages::CompositeLanguage;
use kirin_test_utils::dump_function;
use kirin_test_utils::ir_fixtures::{
    build_add_one, build_div_program, build_infinite_loop, build_linear_program, build_rem_program,
    build_select_program, first_statement_of_specialization,
};

// ===========================================================================
// IR snapshot + concrete select tests (from concrete_interp.rs)
// ===========================================================================

#[test]
fn test_select_ir_snapshot() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_func = build_select_program(&mut pipeline, stage_id);
    let ir = dump_function(spec_func, &pipeline, stage_id);
    insta::assert_snapshot!(ir);
}

#[test]
fn test_concrete_select() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_func = build_select_program(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // select(7) → 7+1 = 8 (truthy: nonzero)
    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_func, &[7])
        .unwrap();
    assert_eq!(result, 8);

    // select(-3) → -3+1 = -2 (truthy: nonzero)
    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_func, &[-3])
        .unwrap();
    assert_eq!(result, -2);

    // select(0) → 42 (falsy: zero)
    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_func, &[0])
        .unwrap();
    assert_eq!(result, 42);
}

// ===========================================================================
// Concrete interpreter tests (from test_dialect_coverage.rs)
// ===========================================================================

#[test]
fn test_concrete_fuel_exhaustion() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_infinite_loop(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> =
        StackInterpreter::new(&pipeline, stage_id).with_fuel(20);

    let result: Result<_, _> = interp.in_stage::<CompositeLanguage>().call(spec_fn, &[42]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, InterpreterError::FuelExhausted),
        "expected FuelExhausted, got: {err:?}"
    );
}

#[test]
fn test_concrete_breakpoints() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let (spec_fn, add_stmt) = build_linear_program(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // Resolve entry and push frame manually for run_until_break
    let stage_info = pipeline.stage(stage_id).unwrap();
    let spec_info = spec_fn.expect_info(stage_info);
    let body_stmt = *spec_info.body();
    let regions: Vec<_> = body_stmt.regions::<CompositeLanguage>(stage_info).collect();
    let blocks: Vec<_> = regions[0].blocks(stage_info).collect();
    let block_info = blocks[0].expect_info(stage_info);
    let first_stmt = block_info.statements.head().copied();

    let frame = Frame::new(spec_fn, stage_id, first_stmt);
    interp.push_frame(frame).unwrap();

    // Set breakpoint at the add statement
    interp.set_breakpoints(FxHashSet::from_iter([add_stmt]));

    // Run until break — should stop before executing add
    let control = interp
        .in_stage::<CompositeLanguage>()
        .run_until_break()
        .unwrap();
    assert!(
        matches!(control, Continuation::Ext(ConcreteExt::Break)),
        "expected Break, got: {control:?}"
    );

    // Clear breakpoints and continue to completion
    interp.clear_breakpoints();
    let control = interp.in_stage::<CompositeLanguage>().run().unwrap();
    match control {
        Continuation::Return(v) => assert_eq!(v, 15, "expected 5 + 10 = 15"),
        other => panic!("expected Return, got: {other:?}"),
    }
}

#[test]
fn test_concrete_push_frame_missing_stage_fails_atomically() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let (spec_fn, _) = build_linear_program(&mut pipeline, stage_id);
    let first_stmt = first_statement_of_specialization(&pipeline, stage_id, spec_fn);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // Build a stage ID that does not exist in this interpreter's pipeline.
    let mut other_pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let _ = other_pipeline.add_stage().stage(StageInfo::default()).new();
    let missing_stage = other_pipeline.add_stage().stage(StageInfo::default()).new();

    let frame = Frame::new(spec_fn, missing_stage, first_stmt);
    let err = interp.push_frame(frame).unwrap_err();
    assert!(
        matches!(err, InterpreterError::StageResolution { stage, .. } if stage == missing_stage),
        "expected MissingStage for pushed frame, got: {err:?}"
    );

    // Failed push must not leave partial frame state behind.
    assert!(
        matches!(interp.pop_frame(), Err(InterpreterError::NoFrame)),
        "failed push should keep stack empty"
    );
}

#[test]
fn test_concrete_push_pop_frame_public_shape() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let (spec_fn, _) = build_linear_program(&mut pipeline, stage_id);
    let first_stmt = first_statement_of_specialization(&pipeline, stage_id, spec_fn);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
    let frame = Frame::new(spec_fn, stage_id, first_stmt);
    interp.push_frame(frame).unwrap();

    let popped = interp.pop_frame().unwrap();
    assert_eq!(popped.callee(), spec_fn);
    assert_eq!(popped.stage(), stage_id);
    assert_eq!(popped.cursor(), first_stmt);
}

#[test]
fn test_concrete_manual_push_then_run_dynamic() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let (spec_fn, _) = build_linear_program(&mut pipeline, stage_id);
    let first_stmt = first_statement_of_specialization(&pipeline, stage_id, spec_fn);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
    let frame = Frame::new(spec_fn, stage_id, first_stmt);
    interp.push_frame(frame).unwrap();

    let control = interp.run().unwrap();
    match control {
        Continuation::Return(v) => assert_eq!(v, 15, "expected 5 + 10 = 15"),
        other => panic!("expected Return, got: {other:?}"),
    }
}

#[test]
fn test_concrete_sequential_calls() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_add_one(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // Call f(5) -> 6
    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_fn, &[5])
        .unwrap();
    assert_eq!(result, 6);

    // Call f(10) -> 11 — interpreter resets between calls
    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_fn, &[10])
        .unwrap();
    assert_eq!(result, 11);

    // Call f(-1) -> 0
    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_fn, &[-1])
        .unwrap();
    assert_eq!(result, 0);
}

#[test]
fn test_concrete_fuel_sufficient() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_add_one(&mut pipeline, stage_id);

    // Enough fuel for a short program
    let mut interp: StackInterpreter<i64, _> =
        StackInterpreter::new(&pipeline, stage_id).with_fuel(100);

    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_fn, &[5])
        .unwrap();
    assert_eq!(result, 6);
}

// ===========================================================================
// Session-style StackInterpreter with Interval (from abstract_interp.rs)
// ===========================================================================

#[test]
fn test_session_abstract_interp_with_args() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let ba_x = b.block_argument().index(0);
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let add = kirin_arith::Arith::<ArithType>::op_add(b, SSAValue::from(ba_x), c1.result);
        let ret = Return::<ArithType>::new(b, vec![add.result.into()]);

        let block = b
            .block()
            .argument(ArithType::I64)
            .stmt(c1)
            .stmt(add)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    });

    // Resolve entry info for manual frame setup
    let stage_info = pipeline.stage(stage_id).unwrap();
    let spec_info = spec_fn.expect_info(stage_info);
    let body_stmt = *spec_info.body();
    let regions: Vec<_> = body_stmt.regions::<CompositeLanguage>(stage_info).collect();
    let blocks: Vec<_> = regions[0].blocks(stage_info).collect();
    let block_info = blocks[0].expect_info(stage_info);
    let first_stmt = block_info.statements.head().copied();
    let block_args: Vec<_> = block_info.arguments.to_vec();

    let call_with = |input: Interval| -> Interval {
        let mut interp: StackInterpreter<Interval, _> = StackInterpreter::new(&pipeline, stage_id);
        let mut frame = Frame::new(spec_fn, stage_id, first_stmt);
        frame.write_ssa(SSAValue::from(block_args[0]), input);
        interp.push_frame(frame).unwrap();
        match interp.in_stage::<CompositeLanguage>().run().unwrap() {
            Continuation::Return(v) => {
                interp.pop_frame().unwrap();
                v
            }
            other => panic!("expected Return, got {:?}", other),
        }
    };

    // [10,20] + [1,1] = [11,21]
    assert_eq!(call_with(Interval::new(10, 20)), Interval::new(11, 21));
    // [0,0] + [1,1] = [1,1]
    assert_eq!(call_with(Interval::constant(0)), Interval::constant(1));
    // top + [1,1] = top
    assert_eq!(call_with(Interval::top()), Interval::top());
    // bottom + anything = bottom
    assert!(call_with(Interval::bottom()).is_empty());
}

// ===========================================================================
// Division by zero error tests
// ===========================================================================

#[test]
fn test_concrete_div_by_zero_returns_error() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_div_program(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // Normal division works
    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_fn, &[10, 2]);
    assert_eq!(result.unwrap(), 5);

    // Division by zero returns an error, not a panic
    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_fn, &[10, 0]);
    assert!(
        result.is_err(),
        "division by zero should return Err, not panic"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, InterpreterError::Custom(_)),
        "expected Custom error for division by zero, got: {err:?}"
    );
}

#[test]
fn test_concrete_rem_by_zero_returns_error() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    let spec_fn = build_rem_program(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);

    // Normal remainder works
    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_fn, &[10, 3]);
    assert_eq!(result.unwrap(), 1);

    // Remainder by zero returns an error, not a panic
    let result = interp
        .in_stage::<CompositeLanguage>()
        .call(spec_fn, &[10, 0]);
    assert!(
        result.is_err(),
        "remainder by zero should return Err, not panic"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, InterpreterError::Custom(_)),
        "expected Custom error for remainder by zero, got: {err:?}"
    );
}

// ===========================================================================
// Multi-result writeback tests
// ===========================================================================

/// A terminator that returns two constant values via `Continuation::Return`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, terminator, type = ArithType, crate = kirin_ir)]
struct MultiReturn {
    val_a: i64,
    val_b: i64,
}

impl<'ir, I> kirin_interpreter::Interpretable<'ir, I> for MultiReturn
where
    I: kirin_interpreter::Interpreter<'ir>,
    I::Value: From<i64> + ProductValue,
{
    fn interpret<L>(&self, _interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: kirin_interpreter::Interpretable<'ir, I> + 'ir,
    {
        let product =
            ProductValue::new_product(vec![I::Value::from(self.val_a), I::Value::from(self.val_b)]);
        Ok(Continuation::Return(product))
    }
}

/// A caller statement that emits `Continuation::Call` with two result slots.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
struct MultiResultCall {
    target: Function,
    callee_stage: CompileStage,
    result_a: ResultValue,
    result_b: ResultValue,
}

impl<'ir, I> kirin_interpreter::Interpretable<'ir, I> for MultiResultCall
where
    I: kirin_interpreter::Interpreter<'ir>,
    I::Value: Clone,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: kirin_interpreter::Interpretable<'ir, I> + 'ir,
    {
        let target_stage = self.callee_stage;
        let stage = interp.resolve_stage_info::<L>(target_stage)?;

        let function_info = interp.pipeline().function_info(self.target).ok_or(
            InterpreterError::StageResolution {
                stage: target_stage,
                kind: kirin_interpreter::StageResolutionError::MissingFunction {
                    function: self.target,
                },
            },
        )?;
        let staged_function = function_info
            .staged_functions()
            .get(&target_stage)
            .copied()
            .ok_or(InterpreterError::StageResolution {
                stage: target_stage,
                kind: kirin_interpreter::StageResolutionError::MissingFunction {
                    function: self.target,
                },
            })?;
        let staged_info =
            staged_function
                .get_info(stage)
                .ok_or(InterpreterError::StageResolution {
                    stage: target_stage,
                    kind: kirin_interpreter::StageResolutionError::MissingFunction {
                        function: self.target,
                    },
                })?;

        let callee = staged_info
            .specializations()
            .iter()
            .find(|spec| !spec.is_invalidated())
            .map(|spec| spec.id())
            .ok_or(InterpreterError::StageResolution {
                stage: target_stage,
                kind: kirin_interpreter::StageResolutionError::NoSpecialization { staged_function },
            })?;

        Ok(Continuation::Call {
            callee,
            stage: target_stage,
            args: smallvec![],
            results: smallvec![self.result_a, self.result_b],
        })
    }
}

/// A terminator that reads two values and sums them into a single Return.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, terminator, type = ArithType, crate = kirin_ir)]
struct SumReturn {
    a: SSAValue,
    b: SSAValue,
}

impl<'ir, I> kirin_interpreter::Interpretable<'ir, I> for SumReturn
where
    I: kirin_interpreter::Interpreter<'ir>,
    I::Value: Clone + std::ops::Add<Output = I::Value>,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: kirin_interpreter::Interpretable<'ir, I> + 'ir,
    {
        let a = interp.read(self.a)?;
        let b = interp.read(self.b)?;
        Ok(Continuation::Return(a + b))
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Dialect,
    kirin_derive_interpreter::Interpretable,
    kirin_derive_interpreter::CallSemantics,
)]
#[wraps]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
enum MultiResultLang {
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    #[kirin(terminator)]
    MultiReturn(MultiReturn),
    MultiResultCall(MultiResultCall),
    #[kirin(terminator)]
    SumReturn(SumReturn),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

#[test]
#[ignore = "requires product-capable value type; will be enabled in Wave 2"]
fn test_multi_result_writeback() {
    let mut pipeline: Pipeline<StageInfo<MultiResultLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    // Build callee: returns (10, 20) via MultiReturn
    let callee_func = pipeline.function().name("callee").new().unwrap();
    let callee_staged = pipeline
        .staged_function::<MultiResultLang>()
        .func(callee_func)
        .stage(stage_id)
        .new()
        .unwrap();

    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let multi_ret = MultiReturn::new(b, 10, 20);
        let block = b.block().terminator(multi_ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize()
            .staged_func(callee_staged)
            .body(body)
            .new()
            .unwrap();
    });

    // Build caller: calls callee, receives two results, sums them, returns sum.
    let caller_func = pipeline.function().name("caller").new().unwrap();
    let caller_staged = pipeline
        .staged_function::<MultiResultLang>()
        .func(caller_func)
        .stage(stage_id)
        .new()
        .unwrap();

    let caller_spec = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let call = MultiResultCall::new(b, callee_func, stage_id);
        let sum_ret = SumReturn::new(
            b,
            SSAValue::from(call.result_a),
            SSAValue::from(call.result_b),
        );
        let block = b.block().stmt(call).terminator(sum_ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize()
            .staged_func(caller_staged)
            .body(body)
            .new()
            .unwrap()
    });

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
    let result = interp.call(caller_spec, stage_id, &[]).unwrap();
    assert_eq!(result, 30, "expected 10 + 20 = 30 from multi-result call");
}

/// A terminator that returns a single value via `Continuation::Return`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, terminator, type = ArithType, crate = kirin_ir)]
struct SingleReturn {
    val: i64,
}

impl<'ir, I> kirin_interpreter::Interpretable<'ir, I> for SingleReturn
where
    I: kirin_interpreter::Interpreter<'ir>,
    I::Value: From<i64>,
{
    fn interpret<L>(&self, _interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: kirin_interpreter::Interpretable<'ir, I> + 'ir,
    {
        Ok(Continuation::Return(I::Value::from(self.val)))
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Dialect,
    kirin_derive_interpreter::Interpretable,
    kirin_derive_interpreter::CallSemantics,
)]
#[wraps]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
enum ArityMismatchLang {
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    MultiResultCall(MultiResultCall),
    #[kirin(terminator)]
    SingleReturn(SingleReturn),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

#[test]
#[ignore = "requires product-capable value type; will be enabled in Wave 2"]
fn test_multi_result_arity_mismatch() {
    // Caller expects 2 results but callee returns 1 — should get ArityMismatch.
    let mut pipeline: Pipeline<StageInfo<ArityMismatchLang>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();

    // Build callee: returns single value (42)
    let callee_func = pipeline.function().name("callee").new().unwrap();
    let callee_staged = pipeline
        .staged_function::<ArityMismatchLang>()
        .func(callee_func)
        .stage(stage_id)
        .new()
        .unwrap();

    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let single_ret = SingleReturn::new(b, 42);
        let block = b.block().terminator(single_ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize()
            .staged_func(callee_staged)
            .body(body)
            .new()
            .unwrap();
    });

    // Build caller: calls callee with 2 result slots but callee returns 1
    let caller_func = pipeline.function().name("caller").new().unwrap();
    let caller_staged = pipeline
        .staged_function::<ArityMismatchLang>()
        .func(caller_func)
        .stage(stage_id)
        .new()
        .unwrap();

    let caller_spec = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let call = MultiResultCall::new(b, callee_func, stage_id);
        let ret = Return::<ArithType>::new(b, vec![SSAValue::from(call.result_a)]);
        let block = b.block().stmt(call).terminator(ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize()
            .staged_func(caller_staged)
            .body(body)
            .new()
            .unwrap()
    });

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
    let err = interp.call(caller_spec, stage_id, &[]).unwrap_err();
    // SingleReturn returns a bare i64 (not a product). When write_statement_results
    // tries to destructure it into 2 result slots, ProductValue::get fails because
    // i64 is not a product type.
    assert!(
        matches!(err, InterpreterError::Custom(_)),
        "expected Custom error for non-product destructure, got: {err:?}"
    );
}
