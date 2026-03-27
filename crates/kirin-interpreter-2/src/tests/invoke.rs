use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_ir::{
    CompileStage, Function, GetInfo, HasArguments, Pipeline, Product, SSAValue, Signature,
    SpecializedFunction, StageInfo, StagedFunction, Symbol, Typeof,
};
use kirin_test_languages::CompositeLanguage;
use kirin_test_utils::ir_fixtures::build_add_one;

use crate::{
    Interpretable, InterpreterError, ProductValue, ValueStore, effect,
    interpreter::{Driver, Invoke, Position, ResolveCallee, SingleStage, TypedStage, callee},
};

#[derive(Clone, Debug, PartialEq)]
enum InvokeValue {
    I64(i64),
    Product(Box<Product<InvokeValue>>),
}

impl Default for InvokeValue {
    fn default() -> Self {
        InvokeValue::I64(0)
    }
}

impl From<ArithValue> for InvokeValue {
    fn from(value: ArithValue) -> Self {
        match value {
            ArithValue::I64(value) => InvokeValue::I64(value),
            _ => panic!("unsupported arith value in invoke test scaffold"),
        }
    }
}

impl std::fmt::Display for InvokeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvokeValue::I64(value) => write!(f, "{value}"),
            InvokeValue::Product(product) => {
                write!(f, "(")?;
                for (index, value) in product.0.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{value}")?;
                }
                write!(f, ")")
            }
        }
    }
}

impl ProductValue for InvokeValue {
    fn as_product(&self) -> Option<&Product<Self>> {
        match self {
            InvokeValue::Product(product) => Some(product.as_ref()),
            _ => None,
        }
    }

    fn from_product(product: Product<Self>) -> Self {
        InvokeValue::Product(Box::new(product))
    }
}

impl Typeof<ArithType> for InvokeValue {
    fn type_of(&self) -> ArithType {
        ArithType::I64
    }
}

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

type InvokeInterp<'ir> = SingleStage<
    'ir,
    CompositeLanguage,
    InvokeValue,
    effect::Stateless<InvokeValue>,
    InterpreterError,
>;

fn as_i64(value: InvokeValue) -> Result<i64, InterpreterError> {
    match value {
        InvokeValue::I64(value) => Ok(value),
        _ => Err(unsupported("expected i64 in invoke scaffold")),
    }
}

fn build_caller_program(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> (SpecializedFunction, SSAValue, SSAValue) {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let x = b.block_argument().index(0);
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let c2 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(2));
        let sum = Arith::<ArithType>::op_add(b, x, c1.result);
        let ret = Return::<ArithType>::new(b, vec![sum.result.into()]);
        let c2_result: SSAValue = c2.result.into();
        let sum_result: SSAValue = sum.result.into();

        let block = b
            .block()
            .argument(ArithType::I64)
            .stmt(c1)
            .stmt(c2)
            .stmt(sum)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        let spec = b.specialize().staged_func(sf).body(body).new().unwrap();
        (spec, c2_result, sum_result)
    })
}

fn build_pair_callee(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();
        let x = b.block_argument().index(0);
        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let dec = Arith::<ArithType>::op_sub(b, x, c1.result);
        let ret = Return::<ArithType>::new(b, vec![x, dec.result.into()]);

        let block = b
            .block()
            .argument(ArithType::I64)
            .stmt(c1)
            .stmt(dec)
            .terminator(ret)
            .new();
        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

fn build_named_constant_body(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_id: CompileStage,
    value: i64,
) -> kirin_ir::Statement {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let constant = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(value));
        let ret = Return::<ArithType>::new(b, vec![constant.result.into()]);
        let block = b.block().stmt(constant).terminator(ret).new();
        let region = b.region().add_block(block).new();
        FunctionBody::<ArithType>::new(b, region, Signature::new(vec![], ArithType::I64, ())).into()
    })
}

struct MultiStageNamedFunction {
    function: Function,
    symbol_a: Symbol,
    symbol_b: Symbol,
    staged_a: StagedFunction,
    staged_b: StagedFunction,
    spec_a: SpecializedFunction,
    spec_b: SpecializedFunction,
}

fn build_multistage_named_function(
    pipeline: &mut Pipeline<StageInfo<CompositeLanguage>>,
    stage_a: CompileStage,
    stage_b: CompileStage,
    name: &str,
) -> MultiStageNamedFunction {
    let body_a = build_named_constant_body(pipeline, stage_a, 11);
    let signature = Signature::new(vec![], ArithType::I64, ());
    let (function, staged_a, spec_a) = pipeline
        .define_function::<CompositeLanguage>()
        .name(name.to_string())
        .stage(stage_a)
        .signature(signature.clone())
        .body(body_a)
        .new()
        .unwrap();

    let body_b = build_named_constant_body(pipeline, stage_b, 22);
    let staged_b = pipeline
        .staged_function::<CompositeLanguage>()
        .func(function)
        .stage(stage_b)
        .signature(signature)
        .new()
        .unwrap();
    let spec_b = pipeline
        .stage_mut(stage_b)
        .unwrap()
        .with_builder(|b| b.specialize().staged_func(staged_b).body(body_b).new())
        .unwrap();

    let symbol_a = pipeline
        .stage_mut(stage_a)
        .unwrap()
        .symbol_table_mut()
        .intern(name.to_string());
    let symbol_b = pipeline
        .stage_mut(stage_b)
        .unwrap()
        .symbol_table_mut()
        .intern(name.to_string());

    MultiStageNamedFunction {
        function,
        symbol_a,
        symbol_b,
        staged_a,
        staged_b,
        spec_a,
        spec_b,
    }
}

impl<'ir> Interpretable<'ir, InvokeInterp<'ir>> for Constant<ArithValue, ArithType> {
    type Machine = effect::Stateless<InvokeValue>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut InvokeInterp<'ir>,
    ) -> Result<effect::Flow<InvokeValue>, Self::Error> {
        interp.write(self.result, self.value.clone().into())?;
        Ok(effect::Flow::Advance)
    }
}

impl<'ir> Interpretable<'ir, InvokeInterp<'ir>> for Arith<ArithType> {
    type Machine = effect::Stateless<InvokeValue>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut InvokeInterp<'ir>,
    ) -> Result<effect::Flow<InvokeValue>, Self::Error> {
        match self {
            Arith::Add {
                lhs, rhs, result, ..
            } => {
                let lhs = as_i64(interp.read(*lhs)?)?;
                let rhs = as_i64(interp.read(*rhs)?)?;
                interp.write(*result, InvokeValue::I64(lhs + rhs))?;
                Ok(effect::Flow::Advance)
            }
            Arith::Sub {
                lhs, rhs, result, ..
            } => {
                let lhs = as_i64(interp.read(*lhs)?)?;
                let rhs = as_i64(interp.read(*rhs)?)?;
                interp.write(*result, InvokeValue::I64(lhs - rhs))?;
                Ok(effect::Flow::Advance)
            }
            _ => Err(unsupported("unsupported arithmetic op in invoke scaffold")),
        }
    }
}

impl<'ir> Interpretable<'ir, InvokeInterp<'ir>> for Return<ArithType> {
    type Machine = effect::Stateless<InvokeValue>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut InvokeInterp<'ir>,
    ) -> Result<effect::Flow<InvokeValue>, Self::Error> {
        let values = self
            .arguments()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(effect::Flow::Stop(InvokeValue::new_product(values)))
    }
}

impl<'ir> Interpretable<'ir, InvokeInterp<'ir>> for FunctionBody<ArithType> {
    type Machine = effect::Stateless<InvokeValue>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        _interp: &mut InvokeInterp<'ir>,
    ) -> Result<effect::Flow<InvokeValue>, Self::Error> {
        Err(unsupported(
            "function bodies are structural and should not be stepped directly",
        ))
    }
}

impl<'ir> Interpretable<'ir, InvokeInterp<'ir>> for CompositeLanguage {
    type Machine = effect::Stateless<InvokeValue>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut InvokeInterp<'ir>,
    ) -> Result<effect::Flow<InvokeValue>, Self::Error> {
        match self {
            CompositeLanguage::Arith(op) => op.interpret(interp),
            CompositeLanguage::Constant(op) => op.interpret(interp),
            CompositeLanguage::FunctionBody(op) => op.interpret(interp),
            CompositeLanguage::Return(op) => op.interpret(interp),
            CompositeLanguage::ControlFlow(_) => {
                Err(unsupported("control flow not used in invoke scaffold"))
            }
        }
    }
}

#[test]
fn invoke_pushes_new_activation_and_preserves_caller_bindings() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_add_one(&mut pipeline, stage_id);
    let (caller, _c2_value, sum_value) = build_caller_program(&mut pipeline, stage_id);

    let mut interp = InvokeInterp::new(&pipeline, stage_id, effect::Stateless::default());
    interp
        .start_specialization(caller, &[InvokeValue::I64(7)])
        .unwrap();

    let entry = interp.entry_block(caller).unwrap();
    let caller_arg = entry.expect_info(interp.stage_info()).arguments[0];
    let caller_statement = interp.current_statement();
    assert!(caller_statement.is_some());
    assert_eq!(interp.read(caller_arg.into()).unwrap(), InvokeValue::I64(7));

    assert!(matches!(
        interp.step().unwrap(),
        crate::result::Step::Stepped(_)
    ));
    let invoke_statement = interp.current_statement().unwrap();
    let resume_statement = (*invoke_statement.next(interp.stage_info())).unwrap();
    assert_ne!(Some(invoke_statement), caller_statement);
    assert_eq!(interp.read(caller_arg.into()).unwrap(), InvokeValue::I64(7));

    let callee_entry = interp.entry_block(callee).unwrap();
    let callee_first = callee_entry.first_statement(interp.stage_info()).unwrap();
    let callee_arg = callee_entry.expect_info(interp.stage_info()).arguments[0];
    let _ = interp.invoke(callee, &[InvokeValue::I64(7)], &[sum_value.into()]);

    assert_eq!(interp.current_statement(), Some(callee_first));
    assert_eq!(
        interp.current_location(),
        Some(crate::control::Location::BeforeStatement(callee_first))
    );
    assert_eq!(interp.read(callee_arg.into()).unwrap(), InvokeValue::I64(7));
    assert!(interp.read(sum_value).is_err());

    let _ = interp.return_current(InvokeValue::I64(8));

    assert_eq!(interp.current_statement(), Some(resume_statement));
    assert_eq!(
        interp.current_location(),
        Some(crate::control::Location::AfterStatement(invoke_statement))
    );
    assert_eq!(interp.read(caller_arg.into()).unwrap(), InvokeValue::I64(7));
    assert_eq!(interp.read(sum_value).unwrap(), InvokeValue::I64(8));
}

#[test]
fn return_current_restores_caller_and_writes_product_results() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee = build_pair_callee(&mut pipeline, stage_id);
    let (caller, c2_value, sum_value) = build_caller_program(&mut pipeline, stage_id);

    let mut interp = InvokeInterp::new(&pipeline, stage_id, effect::Stateless::default());
    interp
        .start_specialization(caller, &[InvokeValue::I64(9)])
        .unwrap();
    assert!(matches!(
        interp.step().unwrap(),
        crate::result::Step::Stepped(_)
    ));
    let invoke_statement = interp.current_statement().unwrap();
    let resume_statement = (*invoke_statement.next(interp.stage_info())).unwrap();

    let callee_entry = interp.entry_block(callee).unwrap();
    let callee_first = callee_entry.first_statement(interp.stage_info()).unwrap();
    let callee_arg = callee_entry.expect_info(interp.stage_info()).arguments[0];

    let _ = interp.invoke(
        callee,
        &[InvokeValue::I64(9)],
        &[sum_value.into(), c2_value.into()],
    );
    assert_eq!(interp.current_statement(), Some(callee_first));
    assert_eq!(
        interp.current_location(),
        Some(crate::control::Location::BeforeStatement(callee_first))
    );
    assert_eq!(interp.read(callee_arg.into()).unwrap(), InvokeValue::I64(9));

    let product = InvokeValue::new_product(vec![InvokeValue::I64(9), InvokeValue::I64(8)]);
    let _ = interp.return_current(product);

    let entry = interp.entry_block(caller).unwrap();
    let caller_arg = entry.expect_info(interp.stage_info()).arguments[0];
    assert_eq!(interp.current_statement(), Some(resume_statement));
    assert_eq!(
        interp.current_location(),
        Some(crate::control::Location::AfterStatement(invoke_statement))
    );
    assert_eq!(interp.read(caller_arg.into()).unwrap(), InvokeValue::I64(9));
    assert_eq!(interp.read(sum_value).unwrap(), InvokeValue::I64(9));
    assert_eq!(interp.read(c2_value).unwrap(), InvokeValue::I64(8));
}

#[test]
fn flow_stay_leaves_current_cursor_unchanged() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let caller = build_add_one(&mut pipeline, stage_id);

    let mut interp = InvokeInterp::new(&pipeline, stage_id, effect::Stateless::default());
    interp
        .start_specialization(caller, &[InvokeValue::I64(3)])
        .unwrap();
    let before_statement = interp.current_statement();
    let before_location = interp.current_location();

    fn stay_effect() -> crate::effect::Flow<InvokeValue> {
        crate::effect::Flow::Stay
    }

    fn apply_flow(interp: &mut InvokeInterp<'_>, flow: crate::effect::Flow<InvokeValue>) {
        interp.apply_control(flow.into_shell()).unwrap();
    }

    apply_flow(&mut interp, stay_effect());
    assert_eq!(interp.current_statement(), before_statement);
    assert_eq!(interp.current_location(), before_location);
}

#[test]
fn callee_builder_resolves_stage_aware_queries() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_a: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage_b: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let target = build_multistage_named_function(&mut pipeline, stage_a, stage_b, "target");
    let interp = InvokeInterp::new(&pipeline, stage_a, effect::Stateless::default());

    assert_eq!(interp.callee().specialized(target.spec_a), target.spec_a);
    assert_eq!(
        interp
            .callee()
            .staged(target.staged_a)
            .specialization(callee::UniqueLive)
            .args(&[])
            .unwrap(),
        target.spec_a
    );
    assert_eq!(
        interp
            .callee()
            .staged(target.staged_b)
            .specialization(callee::UniqueLive)
            .args(&[])
            .unwrap(),
        target.spec_b
    );
    assert_eq!(
        interp
            .callee()
            .function(target.function)
            .stage(stage_b)
            .staged_by(callee::ExactStage)
            .args(&[])
            .unwrap(),
        target.spec_b
    );
    assert_eq!(
        interp.callee().symbol(target.symbol_a).args(&[]).unwrap(),
        target.spec_a
    );
    assert_eq!(
        interp
            .callee()
            .symbol(target.symbol_b)
            .stage(stage_b)
            .specialization(callee::UniqueLive)
            .args(&[])
            .unwrap(),
        target.spec_b
    );
}

#[test]
fn callee_specialization_policy_markers_cover_the_public_policy_surface() {
    assert_eq!(
        callee::SpecializationPolicy::from(callee::UniqueLive),
        callee::SpecializationPolicy::UniqueLive
    );
    assert_eq!(
        callee::SpecializationPolicy::from(callee::ExactMatch),
        callee::SpecializationPolicy::ExactMatch
    );
    assert_eq!(
        callee::SpecializationPolicy::from(callee::BestMatch),
        callee::SpecializationPolicy::BestMatch
    );
    assert_eq!(
        callee::SpecializationPolicy::from(callee::MaterializeExact),
        callee::SpecializationPolicy::MaterializeExact
    );
    assert_eq!(
        callee::SpecializationPolicy::from(callee::MultipleDispatch),
        callee::SpecializationPolicy::MultipleDispatch
    );
}
