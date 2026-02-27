use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::Interpretable;
use kirin_function::FunctionBody;
use kirin_interpreter::{
    BranchCondition, EvalCall, Interpreter, InterpreterError, StackInterpreter,
};
use kirin_ir::*;

// ---------------------------------------------------------------------------
// Dialect with derived Interpretable (all variants are #[wraps]).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable)]
#[wraps]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
pub enum DerivedDialect {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
}

// EvalCall impl (needed by StackInterpreter::call)
impl<'ir, I> EvalCall<'ir, I, DerivedDialect> for DerivedDialect
where
    I: Interpreter<'ir, Error = InterpreterError>,
    I::StageInfo: HasStageInfo<DerivedDialect>,
    I::Value: std::ops::Add<Output = I::Value>
        + std::ops::Sub<Output = I::Value>
        + std::ops::Mul<Output = I::Value>
        + std::ops::Div<Output = I::Value>
        + std::ops::Rem<Output = I::Value>
        + std::ops::Neg<Output = I::Value>
        + From<ArithValue>
        + BranchCondition,
    FunctionBody<ArithType>: EvalCall<'ir, I, DerivedDialect>,
{
    type Result = <FunctionBody<ArithType> as EvalCall<'ir, I, DerivedDialect>>::Result;

    fn eval_call(
        &self,
        interp: &mut I,
        stage: &'ir kirin_ir::StageInfo<DerivedDialect>,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<Self::Result, InterpreterError> {
        match self {
            DerivedDialect::FunctionBody(body) => body.eval_call(interp, stage, callee, args),
            _ => Err(InterpreterError::MissingEntry),
        }
    }
}

// ---------------------------------------------------------------------------
// Build: f(x) = x + 1
// ---------------------------------------------------------------------------

fn build_add_one(
    pipeline: &mut Pipeline<StageInfo<DerivedDialect>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let sf = stage.staged_function().new().unwrap();

    // Entry block with argument x.
    let entry = stage.block().argument(ArithType::I64).new();
    let x: SSAValue = {
        let si = pipeline.stage(stage_id).unwrap();
        let bi = entry.expect_info(si);
        bi.arguments[0].into()
    };

    // Build code block: const 1, add x 1, return sum.
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let sum = Arith::<ArithType>::op_add(stage, x, c1.result);
    let ret = ControlFlow::<ArithType>::op_return(stage, sum.result);
    let code_block = stage.block().stmt(c1).stmt(sum).terminator(ret).new();

    // Add branch from entry to code_block.
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
// End-to-end test: the derived Interpretable (all variants #[wraps])
// produces a working impl that StackInterpreter::call can use.
// ---------------------------------------------------------------------------

#[test]
fn test_derived_interpretable() {
    let mut pipeline: Pipeline<StageInfo<DerivedDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let sf = build_add_one(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<'_, i64, StageInfo<DerivedDialect>> =
        StackInterpreter::new(&pipeline, stage_id);

    let result = interp.call_in_stage::<DerivedDialect>(sf, &[10i64]);
    assert_eq!(result.unwrap(), 11);
}
