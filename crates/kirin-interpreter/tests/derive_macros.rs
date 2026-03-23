use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::{FunctionBody, Return};
use kirin_interpreter::{
    CallSemantics as CallSemanticsTrait, Interpreter, InterpreterError, StackInterpreter,
    StageAccess,
};
use kirin_ir::Dialect;
use kirin_ir::*;

// ---------------------------------------------------------------------------
// Dialect with derived Interpretable + derived CallSemantics using #[callable].
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[wraps]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
pub enum DerivedEvalCallDialect {
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

// ---------------------------------------------------------------------------
// Dialect with derived Interpretable (all variants #[wraps]), manual CallSemantics.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable)]
#[wraps]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
pub enum DerivedInterpretableDialect {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

impl<'ir, I> CallSemanticsTrait<'ir, I> for DerivedInterpretableDialect
where
    I: Interpreter<'ir, Error = InterpreterError>,
    FunctionBody<ArithType>: CallSemanticsTrait<'ir, I>,
{
    type Result = <FunctionBody<ArithType> as CallSemanticsTrait<'ir, I>>::Result;

    fn eval_call<L>(
        &self,
        interp: &mut I,
        stage: &'ir kirin_ir::StageInfo<L>,
        callee: SpecializedFunction,
        args: &[I::Value],
    ) -> Result<Self::Result, InterpreterError>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: kirin_interpreter::Interpretable<'ir, I>
            + CallSemanticsTrait<'ir, I, Result = Self::Result>
            + 'ir,
    {
        match self {
            DerivedInterpretableDialect::FunctionBody(body) => {
                body.eval_call::<L>(interp, stage, callee, args)
            }
            _ => Err(InterpreterError::missing_function_entry()),
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
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        let entry = b.block().argument(ArithType::I64).new();
        let x: SSAValue = b.block_arena()[entry].arguments[0].into();

        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let sum = Arith::<ArithType>::op_add(b, x, c1.result);
        let ret = Return::<ArithType>::new(b, vec![sum.result.into()]);
        let code_block = b.block().stmt(c1).stmt(sum).terminator(ret).new();

        let br = ControlFlow::<ArithType>::op_branch(b, Successor::from_block(code_block), vec![]);
        {
            use kirin_ir::query::ParentInfo;
            let br_stmt: Statement = br.into();
            *b.statement_arena_mut()[br_stmt].get_parent_mut() =
                Some(StatementParent::Block(entry));
            let entry_info = b.block_arena_mut().get_mut(entry).unwrap();
            entry_info.terminator = Some(br_stmt);
        }

        let region = b.region().add_block(entry).add_block(code_block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

fn build_add_one_interpretable(
    pipeline: &mut Pipeline<StageInfo<DerivedInterpretableDialect>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    let stage = pipeline.stage_mut(stage_id).unwrap();
    stage.with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        let entry = b.block().argument(ArithType::I64).new();
        let x: SSAValue = b.block_arena()[entry].arguments[0].into();

        let c1 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(1));
        let sum = Arith::<ArithType>::op_add(b, x, c1.result);
        let ret = Return::<ArithType>::new(b, vec![sum.result.into()]);
        let code_block = b.block().stmt(c1).stmt(sum).terminator(ret).new();

        let br = ControlFlow::<ArithType>::op_branch(b, Successor::from_block(code_block), vec![]);
        {
            use kirin_ir::query::ParentInfo;
            let br_stmt: Statement = br.into();
            *b.statement_arena_mut()[br_stmt].get_parent_mut() =
                Some(StatementParent::Block(entry));
            let entry_info = b.block_arena_mut().get_mut(entry).unwrap();
            entry_info.terminator = Some(br_stmt);
        }

        let region = b.region().add_block(entry).add_block(code_block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            kirin_ir::Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
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
    assert_eq!(result.unwrap()[0], 11);
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
    assert_eq!(result.unwrap()[0], 11);
}
