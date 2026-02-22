mod common;

use common::ArithmeticValue;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::FunctionBody;
use kirin_interpreter::{
    BranchCondition, CallSemantics, Continuation, Interpretable, Interpreter, InterpreterError,
    StackInterpreter,
};
use kirin_ir::*;

// ---------------------------------------------------------------------------
// Dialect with derived CallSemantics (mixed wrapper/non-wrapper).
//
// Only FunctionBody is #[wraps]: the derived CallSemantics forwards to its
// inner SSACFGRegion impl. Arith, ControlFlow, and Constant are non-wrappers:
// the derived CallSemantics returns MissingEntry for them.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, CallSemantics)]
#[kirin(fn, type = ArithType, crate = "kirin_ir")]
pub enum DerivedDialect {
    #[wraps]
    FunctionBody(FunctionBody<ArithType>),
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
}

// From impls for the non-wrapper variants (builders need them)
impl From<Arith<ArithType>> for DerivedDialect {
    fn from(v: Arith<ArithType>) -> Self {
        Self::Arith(v)
    }
}
impl From<ControlFlow<ArithType>> for DerivedDialect {
    fn from(v: ControlFlow<ArithType>) -> Self {
        Self::ControlFlow(v)
    }
}
impl From<Constant<ArithValue, ArithType>> for DerivedDialect {
    fn from(v: Constant<ArithValue, ArithType>) -> Self {
        Self::Constant(v)
    }
}

impl<I> Interpretable<I, Self> for DerivedDialect
where
    I: Interpreter<Error = InterpreterError>,
    I::StageInfo: HasStageInfo<Self>,
    I::Value: ArithmeticValue + BranchCondition,
{
    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Continuation<I::Value, I::Ext>, InterpreterError> {
        match self {
            DerivedDialect::Arith(arith) => match arith {
                Arith::Add {
                    lhs, rhs, result, ..
                } => {
                    let a = interp.read(*lhs)?;
                    let b = interp.read(*rhs)?;
                    interp.write(*result, a.arith_add(&b))?;
                    Ok(Continuation::Continue)
                }
                _ => Err(InterpreterError::MissingEntry),
            },
            DerivedDialect::ControlFlow(cf) => match cf {
                ControlFlow::Branch { target } => Ok(Continuation::Jump((*target).into(), vec![])),
                ControlFlow::Return(value) => {
                    let v = interp.read(*value)?;
                    Ok(Continuation::Return(v))
                }
                ControlFlow::ConditionalBranch {
                    condition,
                    true_target,
                    false_target,
                    ..
                } => {
                    let cond = interp.read(*condition)?;
                    match cond.is_truthy() {
                        Some(true) => Ok(Continuation::Jump((*true_target).into(), vec![])),
                        Some(false) => Ok(Continuation::Jump((*false_target).into(), vec![])),
                        None => Ok(Continuation::Fork(vec![
                            ((*true_target).into(), vec![]),
                            ((*false_target).into(), vec![]),
                        ])),
                    }
                }
            },
            DerivedDialect::Constant(c) => {
                let val = I::Value::from_arith_value(&c.value);
                interp.write(c.result, val)?;
                Ok(Continuation::Continue)
            }
            // FunctionBody is #[wraps] — forward to the inner dialect.
            DerivedDialect::FunctionBody(body) => {
                <FunctionBody<ArithType> as Interpretable<I, Self>>::interpret(body, interp)
            }
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
    let br = ControlFlow::<ArithType>::op_branch(stage, code_block);
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
// End-to-end test: the derived CallSemantics (only FunctionBody is #[wraps])
// produces a working impl that StackInterpreter::call can use.
// ---------------------------------------------------------------------------

#[test]
fn test_derived_call_semantics() {
    let mut pipeline: Pipeline<StageInfo<DerivedDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let sf = build_add_one(&mut pipeline, stage_id);

    let mut interp: StackInterpreter<'_, i64, StageInfo<DerivedDialect>> =
        StackInterpreter::new(&pipeline, stage_id);

    let result = interp.call::<DerivedDialect>(sf, &[10i64]);
    assert_eq!(result.unwrap(), 11);
}
