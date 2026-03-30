use std::{convert::Infallible, ops::ControlFlow};

use kirin_interpreter_3::{
    BranchCondition, Effect, Execute, InterpError, Interpretable, Interpreter, InterpreterError,
    Lift, LiftInto, Machine, PipelineAccess, ProductValue, Project, ResolutionPolicy, TryLift,
    TryLiftInto, TryProject, ValueRead,
};
use kirin_ir::{
    CompileStage, Function, Pipeline, Product, SSAValue, SpecializedFunction, TestSSAValue,
};
use smallvec::smallvec;

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestValue {
    I64(i64),
    Tuple(Box<Product<Self>>),
}

impl ProductValue for TestValue {
    fn as_product(&self) -> Option<&Product<Self>> {
        match self {
            Self::Tuple(product) => Some(product.as_ref()),
            Self::I64(_) => None,
        }
    }

    fn from_product(product: Product<Self>) -> Self {
        Self::Tuple(Box::new(product))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Wrapped {
    Inner(i32),
}

impl Lift<i32> for Wrapped {
    fn lift(from: i32) -> Self {
        Self::Inner(from)
    }
}

impl Project<i32> for Wrapped {
    fn project(self) -> i32 {
        match self {
            Self::Inner(value) => value,
        }
    }
}

struct DummyInterpreter {
    pipeline: Pipeline<()>,
    stage: CompileStage,
}

impl DummyInterpreter {
    fn new() -> Self {
        let mut pipeline = Pipeline::new();
        let stage = pipeline.add_stage_raw(());
        Self { pipeline, stage }
    }
}

impl Machine for DummyInterpreter {
    type Effect = Effect<i64, Infallible>;
    type Error = InterpError<Infallible>;

    fn consume_effect(&mut self, _effect: Self::Effect) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl ValueRead for DummyInterpreter {
    type Value = i64;

    fn read(&self, _value: SSAValue) -> Result<Self::Value, InterpreterError> {
        Ok(7)
    }
}

impl PipelineAccess for DummyInterpreter {
    type StageInfo = ();

    fn pipeline(&self) -> &Pipeline<Self::StageInfo> {
        &self.pipeline
    }

    fn current_stage(&self) -> CompileStage {
        self.stage
    }

    fn resolve_callee(
        &self,
        _function: Function,
        _args: &[i64],
        _policy: ResolutionPolicy,
    ) -> Result<SpecializedFunction, InterpreterError> {
        Err(InterpreterError::unsupported(
            "dummy interpreter does not resolve callees",
        ))
    }
}

impl Interpreter for DummyInterpreter {
    type Dialect = NeverUsedDialect;
    type DialectEffect = Infallible;
    type DialectError = Infallible;

    fn step(&mut self) -> Result<ControlFlow<Self::Value>, Self::Error> {
        Ok(ControlFlow::Break(42))
    }
}

struct DummySeed;

impl Execute<DummyInterpreter> for DummySeed {
    type Output = i64;

    fn execute(
        self,
        _interp: &mut DummyInterpreter,
    ) -> Result<Self::Output, <DummyInterpreter as Machine>::Error> {
        Ok(11)
    }
}

enum NeverUsedDialect {}

impl Interpretable<DummyInterpreter> for NeverUsedDialect {
    type Effect = Infallible;
    type Error = Infallible;

    fn interpret(
        &self,
        _interp: &mut DummyInterpreter,
    ) -> Result<Effect<i64, Self::Effect>, InterpError<Self::Error>> {
        unreachable!("DummyInterpreter::step never calls interpret")
    }
}

#[test]
fn effect_then_builds_ordered_seq() {
    let value = TestValue::I64(1);
    let ssa = SSAValue::from(TestSSAValue(0));
    let effect =
        Effect::<TestValue, Infallible>::BindValue(ssa, value.clone()).then(Effect::Advance);

    assert_eq!(
        effect,
        Effect::Seq(smallvec![
            Box::new(Effect::BindValue(ssa, value)),
            Box::new(Effect::Advance),
        ])
    );
}

#[test]
fn interp_error_wraps_interpreter_error() {
    let error: InterpError<Infallible> = InterpreterError::NoCurrentStatement.into();

    assert_eq!(
        error,
        InterpError::Interpreter(InterpreterError::NoCurrentStatement)
    );
}

#[test]
fn lift_blankets_support_identity_and_wrappers() {
    let wrapped: Wrapped = 3.lift_into();
    let tried_wrapped: Wrapped = 8.try_lift().unwrap();
    let projected: i32 = Wrapped::Inner(9).try_project().unwrap();

    assert_eq!(<i32 as Lift<i32>>::lift(3), 3);
    assert_eq!(wrapped, Wrapped::Inner(3));
    assert_eq!(<Wrapped as Project<i32>>::project(Wrapped::Inner(5)), 5);
    assert_eq!(
        <Wrapped as TryLift<i32>>::try_lift(8).unwrap(),
        Wrapped::Inner(8)
    );
    assert_eq!(tried_wrapped, Wrapped::Inner(8));
    assert_eq!(projected, 9);
}

#[test]
fn product_value_uses_tuple_helpers() {
    let product = TestValue::new_product(vec![TestValue::I64(1), TestValue::I64(2)]);

    assert_eq!(product.len().unwrap(), 2);
    assert_eq!(product.get(0).unwrap(), TestValue::I64(1));
    assert_eq!(product.get(1).unwrap(), TestValue::I64(2));
}

#[test]
fn branch_condition_for_i64_uses_zero_as_false() {
    assert_eq!(0_i64.is_truthy(), Some(false));
    assert_eq!(1_i64.is_truthy(), Some(true));
}

#[test]
fn resolution_policy_defaults_to_unique_live() {
    assert_eq!(ResolutionPolicy::default(), ResolutionPolicy::UniqueLive);
}

#[test]
fn value_read_trait_reads_values() {
    let interp = DummyInterpreter::new();
    let ssa = SSAValue::from(TestSSAValue(1));

    assert_eq!(interp.read(ssa).unwrap(), 7);
}

#[test]
fn pipeline_access_exposes_pipeline_and_stage() {
    let interp = DummyInterpreter::new();

    assert_eq!(interp.pipeline().stage(interp.current_stage()), Some(&()));
}

#[test]
fn pipeline_access_resolve_callee_uses_interpreter_error() {
    let interp = DummyInterpreter::new();
    let mut pipeline: Pipeline<()> = Pipeline::new();
    let function = pipeline.function().new().unwrap();
    let error = interp
        .resolve_callee(function, &[1, 2], ResolutionPolicy::UniqueLive)
        .unwrap_err();

    assert!(matches!(error, InterpreterError::Unsupported(_)));
}

#[test]
fn interpreter_run_uses_default_loop_contract() {
    let mut interp = DummyInterpreter::new();

    let value = interp.run().unwrap();

    assert_eq!(value, 42);
}

#[test]
fn execute_trait_is_usable_for_reusable_entrypoints() {
    let mut interp = DummyInterpreter::new();

    assert_eq!(DummySeed.execute(&mut interp).unwrap(), 11);
}

#[test]
fn machine_trait_consumes_effects() {
    let mut interp = DummyInterpreter::new();

    assert!(interp.consume_effect(Effect::Advance).is_ok());
}
