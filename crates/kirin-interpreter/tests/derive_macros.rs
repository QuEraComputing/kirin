use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::FunctionBody;
use kirin_interpreter::{
    BranchCondition, CallSemantics as CallSemanticsTrait, Interpreter, InterpreterError,
    StackInterpreter,
};
use kirin_ir::*;

// ---------------------------------------------------------------------------
// Dialect with derived Interpretable + derived CallSemantics using #[callable].
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[wraps]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
pub enum DerivedEvalCallDialect {
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
}

// ---------------------------------------------------------------------------
// Dialect with derived Interpretable (all variants #[wraps]), manual CallSemantics.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable)]
#[wraps]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
pub enum DerivedInterpretableDialect {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
}

impl<'ir, I> CallSemanticsTrait<'ir, I, DerivedInterpretableDialect> for DerivedInterpretableDialect
where
    I: Interpreter<'ir, Error = InterpreterError>,
    I::StageInfo: HasStageInfo<DerivedInterpretableDialect>,
    I::Value: std::ops::Add<Output = I::Value>
        + std::ops::Sub<Output = I::Value>
        + std::ops::Mul<Output = I::Value>
        + std::ops::Div<Output = I::Value>
        + std::ops::Rem<Output = I::Value>
        + std::ops::Neg<Output = I::Value>
        + From<ArithValue>
        + BranchCondition,
    FunctionBody<ArithType>: CallSemanticsTrait<'ir, I, DerivedInterpretableDialect>,
{
    type Result = <FunctionBody<ArithType> as CallSemanticsTrait<
        'ir,
        I,
        DerivedInterpretableDialect,
    >>::Result;

    fn eval_call(
        &self,
        interp: &mut I,
        stage: &'ir kirin_ir::StageInfo<DerivedInterpretableDialect>,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<Self::Result, InterpreterError> {
        match self {
            DerivedInterpretableDialect::FunctionBody(body) => {
                body.eval_call(interp, stage, callee, args)
            }
            _ => Err(InterpreterError::MissingEntry),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared builder: f(x) = x + 1 (using a given dialect type)
// ---------------------------------------------------------------------------

fn build_add_one_eval_call(
    pipeline: &mut Pipeline<StageInfo<DerivedEvalCallDialect>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let entry = stage.block().argument(ArithType::I64).new();
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        let bi = entry.expect_info(si);
        bi.arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let sum = Arith::<ArithType>::op_add(stage, x, c1.result);
    let ret = ControlFlow::<ArithType>::op_return(stage, sum.result);
    let code_block = stage.block().stmt(c1).stmt(sum).terminator(ret).new();

    let br = ControlFlow::<ArithType>::op_branch(stage, Successor::from_block(code_block), vec![]);
    {
        use kirin_ir::query::ParentInfo;
        let br_stmt: Statement = br.into();
        *br_stmt.expect_info_mut(stage).get_parent_mut() = Some(entry);
        let entry_info = entry.get_info_mut(stage).unwrap();
        entry_info.terminator = Some(br_stmt);
    }

    let region = stage.region().add_block(entry).add_block(code_block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(body).new().unwrap()
}

fn build_add_one_interpretable(
    pipeline: &mut Pipeline<StageInfo<DerivedInterpretableDialect>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    let entry = stage.block().argument(ArithType::I64).new();
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        let bi = entry.expect_info(si);
        bi.arguments[0].into()
    };

    let stage = pipeline.stage_mut(stage_id).unwrap();
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let sum = Arith::<ArithType>::op_add(stage, x, c1.result);
    let ret = ControlFlow::<ArithType>::op_return(stage, sum.result);
    let code_block = stage.block().stmt(c1).stmt(sum).terminator(ret).new();

    let br = ControlFlow::<ArithType>::op_branch(stage, Successor::from_block(code_block), vec![]);
    {
        use kirin_ir::query::ParentInfo;
        let br_stmt: Statement = br.into();
        *br_stmt.expect_info_mut(stage).get_parent_mut() = Some(entry);
        let entry_info = entry.get_info_mut(stage).unwrap();
        entry_info.terminator = Some(br_stmt);
    }

    let region = stage.region().add_block(entry).add_block(code_block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    stage.specialize().f(sf).body(body).new().unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_derived_eval_call() {
    let mut pipeline: Pipeline<StageInfo<DerivedEvalCallDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let sf = build_add_one_eval_call(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<'_, i64, StageInfo<DerivedEvalCallDialect>> =
        StackInterpreter::new(&pipeline, stage_id);

    let result = interp
        .in_stage::<DerivedEvalCallDialect>()
        .call(sf, &[10i64]);
    assert_eq!(result.unwrap(), 11);
}

#[test]
fn test_derived_interpretable() {
    let mut pipeline: Pipeline<StageInfo<DerivedInterpretableDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let sf = build_add_one_interpretable(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<'_, i64, StageInfo<DerivedInterpretableDialect>> =
        StackInterpreter::new(&pipeline, stage_id);

    let result = interp
        .in_stage::<DerivedInterpretableDialect>()
        .call(sf, &[10i64]);
    assert_eq!(result.unwrap(), 11);
}
