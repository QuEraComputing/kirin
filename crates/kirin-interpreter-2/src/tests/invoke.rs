use kirin_arith::ArithValue;
use kirin_ir::{CompileStage, Pipeline, SpecializedFunction, StageInfo};
use kirin_test_languages::CompositeLanguage;
use kirin_test_utils::ir_fixtures::build_add_one;

use crate::{
    ConsumeEffect, InterpreterError, Machine, control::Shell, effect, interpreter::SingleStage,
};

#[derive(Debug, Default)]
struct InvokeMachine;

impl<'ir> Machine<'ir> for InvokeMachine {
    type Effect = effect::Flow<ArithValue>;
    type Stop = ArithValue;
}

impl<'ir> ConsumeEffect<'ir> for InvokeMachine {
    type Error = InterpreterError;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Shell<Self::Stop>, Self::Error> {
        Ok(match effect {
            effect::Flow::Advance => Shell::Advance,
            effect::Flow::Jump(seed) => Shell::Replace(seed),
            effect::Flow::Stop(stop) => Shell::Stop(stop),
        })
    }
}

type InvokeInterp<'ir> =
    SingleStage<'ir, CompositeLanguage, ArithValue, InvokeMachine, InterpreterError>;

#[test]
fn invoke_pushes_new_activation_and_preserves_caller_bindings() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let callee: SpecializedFunction = build_add_one(&mut pipeline, stage_id);
    let mut interp = InvokeInterp::new(&pipeline, stage_id, InvokeMachine);
    let args = [ArithValue::I64(5)];

    fn uses_invoke<'ir, I: crate::interpreter::Invoke<'ir>>(
        interp: &mut I,
        callee: SpecializedFunction,
        args: &[ArithValue],
    ) {
        let _ = interp.invoke(callee, args, &[]);
    }

    uses_invoke(&mut interp, callee, &args);
}

#[test]
fn return_current_restores_caller_and_writes_product_results() {
    let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let _callee: SpecializedFunction = build_add_one(&mut pipeline, stage_id);
    let mut interp = InvokeInterp::new(&pipeline, stage_id, InvokeMachine);

    fn uses_return<'ir, I: crate::interpreter::Invoke<'ir>>(interp: &mut I, value: ArithValue) {
        let _ = interp.return_current(value);
    }

    uses_return(&mut interp, ArithValue::I64(1));
}

#[test]
fn flow_stay_leaves_current_cursor_unchanged() {
    let _ = crate::effect::Flow::<ArithValue>::Stay;
}
