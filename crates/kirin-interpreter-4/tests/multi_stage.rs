use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_interpreter_4::concrete::{Action, Boxed, MultiStage};
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::traits::{Interpretable, ValueStore};
use kirin_ir::*;

// ---------------------------------------------------------------------------
// TestValue — shared value type for all multi-stage tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum TestValue {
    I64(i64),
}

// ---------------------------------------------------------------------------
// DialectA — Constant + FunctionBody + Return + CrossStageCall
// ---------------------------------------------------------------------------

/// Custom call op that stores the callee + stage directly (no symbol resolution).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
struct CrossStageCall {
    callee: SpecializedFunction,
    callee_stage: CompileStage,
    args: Vec<SSAValue>,
    result: ResultValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[wraps]
enum DialectA {
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
    CrossCall(CrossStageCall),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

// ---------------------------------------------------------------------------
// DialectB — Constant + FunctionBody + Return + Negate
// ---------------------------------------------------------------------------

/// Negates an i64 value: result = -arg.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
struct Negate {
    arg: SSAValue,
    result: ResultValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[wraps]
enum DialectB {
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
    Negate(Negate),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

// ---------------------------------------------------------------------------
// TwoStage — a two-dialect stage container
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, StageMeta)]
#[stage(crate = "kirin_ir")]
enum TwoStage {
    #[stage(name = "a")]
    A(StageInfo<DialectA>),
    #[stage(name = "b")]
    B(StageInfo<DialectB>),
}

// ---------------------------------------------------------------------------
// Type aliases for conciseness
// ---------------------------------------------------------------------------

type TwoStageInterp<'ir> = MultiStage<'ir, TwoStage, TestValue>;
type TwoStageAction<'ir> = Action<TestValue, (), Boxed<'ir, TwoStageInterp<'ir>>>;

// ---------------------------------------------------------------------------
// Interpretable impls for DialectA
// ---------------------------------------------------------------------------

fn read_i64<I: ValueStore<Value = TestValue, Error = InterpreterError>>(
    interp: &I,
    ssa: SSAValue,
) -> Result<i64, InterpreterError> {
    match interp.read(ssa)? {
        TestValue::I64(n) => Ok(n),
    }
}

impl<'ir> Interpretable<TwoStageInterp<'ir>> for DialectA {
    type Effect = TwoStageAction<'ir>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut TwoStageInterp<'ir>,
    ) -> Result<TwoStageAction<'ir>, InterpreterError> {
        match self {
            DialectA::Constant(c) => {
                let val = match &c.value {
                    ArithValue::I64(n) => TestValue::I64(*n),
                    other => return Err(InterpreterError::UnhandledEffect(format!("{other:?}"))),
                };
                interp.write(c.result, val)?;
                Ok(Action::Advance)
            }
            DialectA::Return(ret) => {
                let val = ret
                    .arguments()
                    .next()
                    .map(|ssa| interp.read(*ssa))
                    .transpose()?
                    .unwrap_or(TestValue::I64(0));
                Ok(Action::Return(val))
            }
            DialectA::CrossCall(cc) => {
                let args = interp.read_many(&cc.args)?;
                Ok(Action::Call(
                    cc.callee,
                    cc.callee_stage,
                    args,
                    vec![cc.result],
                ))
            }
            DialectA::FunctionBody(_) => Err(InterpreterError::UnhandledEffect(
                "FunctionBody in DialectA".into(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Interpretable impls for DialectB
// ---------------------------------------------------------------------------

impl<'ir> Interpretable<TwoStageInterp<'ir>> for DialectB {
    type Effect = TwoStageAction<'ir>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut TwoStageInterp<'ir>,
    ) -> Result<TwoStageAction<'ir>, InterpreterError> {
        match self {
            DialectB::Constant(c) => {
                let val = match &c.value {
                    ArithValue::I64(n) => TestValue::I64(*n),
                    other => return Err(InterpreterError::UnhandledEffect(format!("{other:?}"))),
                };
                interp.write(c.result, val)?;
                Ok(Action::Advance)
            }
            DialectB::Negate(neg) => {
                let n = read_i64(interp, neg.arg)?;
                interp.write(neg.result, TestValue::I64(-n))?;
                Ok(Action::Advance)
            }
            DialectB::Return(ret) => {
                let val = ret
                    .arguments()
                    .next()
                    .map(|ssa| interp.read(*ssa))
                    .transpose()?
                    .unwrap_or(TestValue::I64(0));
                Ok(Action::Return(val))
            }
            DialectB::FunctionBody(_) => Err(InterpreterError::UnhandledEffect(
                "FunctionBody in DialectB".into(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a simple single-stage-A program: `constant K; return %0`.
fn build_stage_a_constant(
    pipeline: &mut Pipeline<TwoStage>,
    stage_id: CompileStage,
    k: i64,
) -> (SpecializedFunction, Block) {
    let TwoStage::A(stage) = pipeline.stage_mut(stage_id).unwrap() else {
        panic!("expected stage A");
    };
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let c = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(k));
        let ret = Return::<ArithType>::new(b, vec![c.result.into()]);
        let block = b.block().stmt(c).terminator(ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        let spec = b.specialize().staged_func(sf).body(body).new().unwrap();
        (spec, block)
    })
}

/// Build a stage-B function: `constant K; negate %0; return %1`.
fn build_stage_b_negate_const(
    pipeline: &mut Pipeline<TwoStage>,
    stage_id: CompileStage,
    k: i64,
) -> (SpecializedFunction, Block) {
    let TwoStage::B(stage) = pipeline.stage_mut(stage_id).unwrap() else {
        panic!("expected stage B");
    };
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let c = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(k));
        let neg = Negate::new(b, SSAValue::from(c.result));
        let ret = Return::<ArithType>::new(b, vec![neg.result.into()]);
        let block = b.block().stmt(c).stmt(neg).terminator(ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        let spec = b.specialize().staged_func(sf).body(body).new().unwrap();
        (spec, block)
    })
}

/// Build a stage-A caller function that calls a stage-B callee, returns the result.
fn build_stage_a_caller(
    pipeline: &mut Pipeline<TwoStage>,
    stage_id: CompileStage,
    callee: SpecializedFunction,
    callee_stage: CompileStage,
) -> (SpecializedFunction, Block) {
    let TwoStage::A(stage) = pipeline.stage_mut(stage_id).unwrap() else {
        panic!("expected stage A");
    };
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let cc = CrossStageCall::new(b, callee, callee_stage, vec![]);
        let ret = Return::<ArithType>::new(b, vec![cc.result.into()]);
        let block = b.block().stmt(cc).terminator(ret).new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        let spec = b.specialize().staged_func(sf).body(body).new().unwrap();
        (spec, block)
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// MultiStage can run a single-dialect-A program (same as SingleStage but via MultiStage).
#[test]
fn test_multi_stage_single_dialect_run() {
    let mut pipeline: Pipeline<TwoStage> = Pipeline::new();
    let stage_a = pipeline
        .add_stage()
        .stage(TwoStage::A(StageInfo::default()))
        .new();

    let (spec, entry_block) = build_stage_a_constant(&mut pipeline, stage_a, 42);

    let mut interp = TwoStageInterp::new(&pipeline, stage_a, ());
    interp
        .enter_function::<DialectA>(spec, entry_block, &[])
        .unwrap();
    let result = interp.run().unwrap();

    assert_eq!(result, Some(TestValue::I64(42)));
}

/// MultiStage can run a single-dialect-B program (Negate op).
#[test]
fn test_multi_stage_dialect_b_negate() {
    let mut pipeline: Pipeline<TwoStage> = Pipeline::new();
    let stage_b = pipeline
        .add_stage()
        .stage(TwoStage::B(StageInfo::default()))
        .new();

    let (spec, entry_block) = build_stage_b_negate_const(&mut pipeline, stage_b, 7);

    let mut interp = TwoStageInterp::new(&pipeline, stage_b, ());
    interp
        .enter_function::<DialectB>(spec, entry_block, &[])
        .unwrap();
    let result = interp.run().unwrap();

    assert_eq!(result, Some(TestValue::I64(-7)));
}

/// Cross-stage call: StageA calls a StageB function and returns its result.
#[test]
fn test_cross_stage_call() {
    let mut pipeline: Pipeline<TwoStage> = Pipeline::new();
    let stage_a = pipeline
        .add_stage()
        .stage(TwoStage::A(StageInfo::default()))
        .new();
    let stage_b = pipeline
        .add_stage()
        .stage(TwoStage::B(StageInfo::default()))
        .new();

    // Build callee in StageB: constant 15 → negate → return -15
    let (callee_spec, _) = build_stage_b_negate_const(&mut pipeline, stage_b, 15);

    // Build caller in StageA: CrossCall(callee_spec, stage_b) → return result
    let (caller_spec, caller_entry) =
        build_stage_a_caller(&mut pipeline, stage_a, callee_spec, stage_b);

    let mut interp = TwoStageInterp::new(&pipeline, stage_a, ());
    interp
        .enter_function::<DialectA>(caller_spec, caller_entry, &[])
        .unwrap();
    let result = interp.run().unwrap();

    assert_eq!(result, Some(TestValue::I64(-15)));
}

/// Both stages present; a two-call chain A → B → A works correctly.
///
/// Program:
///   StageB helper_b: constant 10 → negate → return -10
///   StageA caller_a: call helper_b → return result (-10)
///   StageA outer_a:  call caller_a → return result (-10)
#[test]
fn test_nested_cross_stage_calls() {
    let mut pipeline: Pipeline<TwoStage> = Pipeline::new();
    let stage_a = pipeline
        .add_stage()
        .stage(TwoStage::A(StageInfo::default()))
        .new();
    let stage_b = pipeline
        .add_stage()
        .stage(TwoStage::B(StageInfo::default()))
        .new();

    let (helper_b_spec, _) = build_stage_b_negate_const(&mut pipeline, stage_b, 10);
    let (caller_a_spec, _) = build_stage_a_caller(&mut pipeline, stage_a, helper_b_spec, stage_b);
    let (outer_a_spec, outer_entry) =
        build_stage_a_caller(&mut pipeline, stage_a, caller_a_spec, stage_a);

    let mut interp = TwoStageInterp::new(&pipeline, stage_a, ());
    interp
        .enter_function::<DialectA>(outer_a_spec, outer_entry, &[])
        .unwrap();
    let result = interp.run().unwrap();

    assert_eq!(result, Some(TestValue::I64(-10)));
}

/// Step-by-step execution with MultiStage.
#[test]
fn test_multi_stage_step_by_step() {
    let mut pipeline: Pipeline<TwoStage> = Pipeline::new();
    let stage_a = pipeline
        .add_stage()
        .stage(TwoStage::A(StageInfo::default()))
        .new();

    let (spec, entry_block) = build_stage_a_constant(&mut pipeline, stage_a, 99);

    let mut interp = TwoStageInterp::new(&pipeline, stage_a, ());
    interp
        .enter_function::<DialectA>(spec, entry_block, &[])
        .unwrap();

    assert!(interp.step().unwrap(), "first step executes block");
    assert!(!interp.step().unwrap(), "second step: stack empty");
}
