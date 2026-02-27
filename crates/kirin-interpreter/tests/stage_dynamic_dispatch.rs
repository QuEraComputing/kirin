use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{EvalCall, Interpretable};
use kirin_function::FunctionBody;
use kirin_interpreter::{Continuation, InterpreterError, StackInterpreter};
use kirin_ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(fn, type = T, crate = kirin_ir)]
struct StageCall<T: CompileTimeValue + Default> {
    target: Function,
    callee_stage: CompileStage,
    args: Vec<SSAValue>,
    #[kirin(type = T::default())]
    result: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue + Default> StageCall<T> {
    fn target(&self) -> Function {
        self.target
    }

    fn target_stage(&self) -> CompileStage {
        self.callee_stage
    }

    fn args(&self) -> &[SSAValue] {
        &self.args
    }

    fn result(&self) -> ResultValue {
        self.result
    }
}

impl<'ir, I, L, T> kirin_interpreter::Interpretable<'ir, I, L> for StageCall<T>
where
    I: kirin_interpreter::Interpreter<'ir>,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    I::Value: Clone,
    L: Dialect + 'ir,
    T: CompileTimeValue + Default,
{
    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Continuation<I::Value, I::Ext>, <I as kirin_interpreter::Interpreter<'ir>>::Error>
    {
        let target_stage = self.target_stage();
        let stage_meta =
            interp
                .pipeline()
                .stage(target_stage)
                .ok_or(InterpreterError::MissingStage {
                    stage: target_stage,
                })?;
        let stage = <I::StageInfo as HasStageInfo<L>>::try_stage_info(stage_meta).ok_or(
            InterpreterError::MissingStageDialect {
                stage: target_stage,
            },
        )?;

        let function_info = interp.pipeline().function_info(self.target()).ok_or(
            InterpreterError::MissingFunctionStageMapping {
                function: self.target(),
                stage: target_stage,
            },
        )?;
        let staged_function = function_info
            .staged_functions()
            .get(&target_stage)
            .copied()
            .ok_or(InterpreterError::MissingFunctionStageMapping {
                function: self.target(),
                stage: target_stage,
            })?;
        let staged_info = staged_function.get_info(stage).ok_or(
            InterpreterError::MissingFunctionStageMapping {
                function: self.target(),
                stage: target_stage,
            },
        )?;

        let mut live = staged_info
            .specializations()
            .iter()
            .filter(|spec| !spec.is_invalidated())
            .map(|spec| spec.id());
        let callee = match (live.next(), live.next()) {
            (None, _) => {
                return Err(InterpreterError::NoSpecializationAtStage {
                    staged_function,
                    stage: target_stage,
                }
                .into());
            }
            (Some(callee), None) => callee,
            (Some(_), Some(_)) => {
                let count = staged_info
                    .specializations()
                    .iter()
                    .filter(|spec| !spec.is_invalidated())
                    .count();
                return Err(InterpreterError::AmbiguousSpecializationAtStage {
                    staged_function,
                    stage: target_stage,
                    count,
                }
                .into());
            }
        };

        let args = self
            .args()
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<kirin_interpreter::Args<I::Value>, _>>()?;

        Ok(Continuation::Call {
            callee,
            stage: target_stage,
            args,
            result: self.result(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, EvalCall)]
#[wraps]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
enum StageDynLang {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    StageCall(StageCall<ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, EvalCall)]
#[wraps]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
enum FunctionCallLang {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    Call(kirin_function::Call<ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
}

fn specialize_return_const(
    stage: &mut StageInfo<FunctionCallLang>,
    staged_function: StagedFunction,
    value: i64,
    with_arg: bool,
) -> SpecializedFunction {
    let c = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(value));
    let ret = ControlFlow::<ArithType>::op_return(stage, c.result);
    let block = stage.block().stmt(c).terminator(ret).new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    if with_arg {
        stage
            .specialize()
            .f(staged_function)
            .signature(Signature {
                params: vec![ArithType::I64],
                ret: ArithType::I64,
                constraints: (),
            })
            .body(body)
            .new()
            .unwrap()
    } else {
        stage
            .specialize()
            .f(staged_function)
            .body(body)
            .new()
            .unwrap()
    }
}

fn build_cross_stage_recursive_body(
    stage: &mut StageInfo<StageDynLang>,
    target_func: Function,
    target_stage: CompileStage,
) -> Statement {
    use kirin_ir::query::ParentInfo;

    let entry = stage.block().argument(ArithType::I64).new();
    let call_block = stage.block().argument(ArithType::I64).new();
    let exit_block = stage.block().new();

    let n: SSAValue = entry.expect_info(stage).arguments[0].into();
    let call_arg: SSAValue = call_block.expect_info(stage).arguments[0].into();

    let c0 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(0));
    let ret0 = ControlFlow::<ArithType>::op_return(stage, c0.result);
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

    let call = StageCall::<ArithType>::new(stage, target_func, target_stage, vec![call_arg]);
    let ret = ControlFlow::<ArithType>::op_return(stage, call.result);
    {
        let call_stmt: Statement = call.into();
        *call_stmt.expect_info_mut(stage).get_parent_mut() = Some(call_block);
        let linked = stage.link_statements(&[call_stmt]);
        let ret_stmt: Statement = ret.into();
        *ret_stmt.expect_info_mut(stage).get_parent_mut() = Some(call_block);
        let call_info = call_block.get_info_mut(stage).unwrap();
        call_info.statements = linked;
        call_info.terminator = Some(ret_stmt);
    }

    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let dec = Arith::<ArithType>::op_sub(stage, n, c1.result);
    let cond = ControlFlow::<ArithType>::op_conditional_branch(
        stage,
        n,
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
    FunctionBody::<ArithType>::new(stage, region).into()
}

#[test]
fn test_cross_stage_recursive_call() {
    let mut pipeline: Pipeline<StageInfo<StageDynLang>> = Pipeline::new();
    let stage_a = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage_b = pipeline.add_stage().stage(StageInfo::default()).new();

    let func = pipeline.function().name("rec").new();
    let staged_a = pipeline
        .staged_function::<StageDynLang>()
        .func(func)
        .stage(stage_a)
        .new()
        .unwrap();
    let staged_b = pipeline
        .staged_function::<StageDynLang>()
        .func(func)
        .stage(stage_b)
        .new()
        .unwrap();

    let spec_a = {
        let stage = pipeline.stage_mut(stage_a).unwrap();
        let body = build_cross_stage_recursive_body(stage, func, stage_b);
        stage.specialize().f(staged_a).body(body).new().unwrap()
    };
    {
        let stage = pipeline.stage_mut(stage_b).unwrap();
        let body = build_cross_stage_recursive_body(stage, func, stage_a);
        stage.specialize().f(staged_b).body(body).new().unwrap();
    }

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_a);
    let result = interp.call(spec_a, stage_a, &[6]).unwrap();
    assert_eq!(result, 0);
}

fn build_caller_with_function_call(
    stage: &mut StageInfo<FunctionCallLang>,
    target: &str,
) -> Statement {
    let target_symbol = stage.symbol_table_mut().intern(target.to_string());
    let call = kirin_function::Call::<ArithType>::new(stage, target_symbol, vec![]);
    let ret = ControlFlow::<ArithType>::op_return(stage, call.res);
    let block = stage.block().stmt(call).terminator(ret).new();
    let region = stage.region().add_block(block).new();
    FunctionBody::<ArithType>::new(stage, region).into()
}

#[test]
fn test_function_call_missing_stage_mapping_error() {
    let mut pipeline: Pipeline<StageInfo<FunctionCallLang>> = Pipeline::new();
    let stage_a = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage_b = pipeline.add_stage().stage(StageInfo::default()).new();

    let callee_func = pipeline.function().name("callee").new();
    let caller_func = pipeline.function().name("caller").new();

    let callee_staged_a = pipeline
        .staged_function::<FunctionCallLang>()
        .func(callee_func)
        .stage(stage_a)
        .new()
        .unwrap();
    let caller_staged_b = pipeline
        .staged_function::<FunctionCallLang>()
        .func(caller_func)
        .stage(stage_b)
        .new()
        .unwrap();

    {
        let stage = pipeline.stage_mut(stage_a).unwrap();
        specialize_return_const(stage, callee_staged_a, 7, false);
    }

    let caller_spec = {
        let stage = pipeline.stage_mut(stage_b).unwrap();
        let body = build_caller_with_function_call(stage, "callee");
        stage
            .specialize()
            .f(caller_staged_b)
            .body(body)
            .new()
            .unwrap()
    };

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_b);
    let err = interp.call(caller_spec, stage_b, &[]).unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::MissingFunctionStageMapping {
                function,
                stage
            } if function == callee_func && stage == stage_b
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn test_function_call_no_specialization_error() {
    let mut pipeline: Pipeline<StageInfo<FunctionCallLang>> = Pipeline::new();
    let stage = pipeline.add_stage().stage(StageInfo::default()).new();

    let callee_func = pipeline.function().name("callee").new();
    let caller_func = pipeline.function().name("caller").new();

    let _callee_staged = pipeline
        .staged_function::<FunctionCallLang>()
        .func(callee_func)
        .stage(stage)
        .new()
        .unwrap();
    let caller_staged = pipeline
        .staged_function::<FunctionCallLang>()
        .func(caller_func)
        .stage(stage)
        .new()
        .unwrap();

    let caller_spec = {
        let stage_info = pipeline.stage_mut(stage).unwrap();
        let body = build_caller_with_function_call(stage_info, "callee");
        stage_info
            .specialize()
            .f(caller_staged)
            .body(body)
            .new()
            .unwrap()
    };

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage);
    let err = interp.call(caller_spec, stage, &[]).unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::NoSpecializationAtStage {
                stage: err_stage,
                ..
            } if err_stage == stage
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn test_function_call_ambiguous_specialization_error() {
    let mut pipeline: Pipeline<StageInfo<FunctionCallLang>> = Pipeline::new();
    let stage = pipeline.add_stage().stage(StageInfo::default()).new();

    let callee_func = pipeline.function().name("callee").new();
    let caller_func = pipeline.function().name("caller").new();

    let callee_staged = pipeline
        .staged_function::<FunctionCallLang>()
        .func(callee_func)
        .stage(stage)
        .new()
        .unwrap();
    let caller_staged = pipeline
        .staged_function::<FunctionCallLang>()
        .func(caller_func)
        .stage(stage)
        .new()
        .unwrap();

    {
        let stage_info = pipeline.stage_mut(stage).unwrap();
        specialize_return_const(stage_info, callee_staged, 3, false);
        specialize_return_const(stage_info, callee_staged, 5, true);
    }

    let caller_spec = {
        let stage_info = pipeline.stage_mut(stage).unwrap();
        let body = build_caller_with_function_call(stage_info, "callee");
        stage_info
            .specialize()
            .f(caller_staged)
            .body(body)
            .new()
            .unwrap()
    };

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage);
    let err = interp.call(caller_spec, stage, &[]).unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::AmbiguousSpecializationAtStage {
                stage: err_stage,
                count,
                ..
            } if err_stage == stage && count == 2
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn test_function_call_unique_specialization_success() {
    let mut pipeline: Pipeline<StageInfo<FunctionCallLang>> = Pipeline::new();
    let stage = pipeline.add_stage().stage(StageInfo::default()).new();

    let callee_func = pipeline.function().name("callee").new();
    let caller_func = pipeline.function().name("caller").new();

    let callee_staged = pipeline
        .staged_function::<FunctionCallLang>()
        .func(callee_func)
        .stage(stage)
        .new()
        .unwrap();
    let caller_staged = pipeline
        .staged_function::<FunctionCallLang>()
        .func(caller_func)
        .stage(stage)
        .new()
        .unwrap();

    {
        let stage_info = pipeline.stage_mut(stage).unwrap();
        specialize_return_const(stage_info, callee_staged, 11, false);
    }

    let caller_spec = {
        let stage_info = pipeline.stage_mut(stage).unwrap();
        let body = build_caller_with_function_call(stage_info, "callee");
        stage_info
            .specialize()
            .f(caller_staged)
            .body(body)
            .new()
            .unwrap()
    };

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage);
    let result = interp.call(caller_spec, stage, &[]).unwrap();
    assert_eq!(result, 11);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, EvalCall)]
#[wraps]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
enum DummyLang {
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
}

#[derive(Debug, Clone)]
enum MixedStage {
    Dyn(StageInfo<StageDynLang>),
    Dummy(StageInfo<DummyLang>),
}

impl StageMeta for MixedStage {
    type Languages = (StageDynLang, (DummyLang, ()));

    fn stage_name(&self) -> Option<GlobalSymbol> {
        match self {
            MixedStage::Dyn(stage) => stage.name(),
            MixedStage::Dummy(stage) => stage.name(),
        }
    }

    fn set_stage_name(&mut self, name: Option<GlobalSymbol>) {
        match self {
            MixedStage::Dyn(stage) => stage.set_name(name),
            MixedStage::Dummy(stage) => stage.set_name(name),
        }
    }

    fn stage_id(&self) -> Option<CompileStage> {
        match self {
            MixedStage::Dyn(stage) => stage.stage_id(),
            MixedStage::Dummy(stage) => stage.stage_id(),
        }
    }

    fn set_stage_id(&mut self, id: Option<CompileStage>) {
        match self {
            MixedStage::Dyn(stage) => stage.set_stage_id(id),
            MixedStage::Dummy(stage) => stage.set_stage_id(id),
        }
    }

    fn from_stage_name(stage_name: &str) -> Result<Self, String> {
        match stage_name {
            "dyn" => Ok(MixedStage::Dyn(StageInfo::default())),
            "dummy" => Ok(MixedStage::Dummy(StageInfo::default())),
            _ => Err(format!("unknown stage name: {stage_name}")),
        }
    }

    fn declared_stage_names() -> &'static [&'static str] {
        &["dyn", "dummy"]
    }
}

impl HasStageInfo<StageDynLang> for MixedStage {
    fn try_stage_info(&self) -> Option<&StageInfo<StageDynLang>> {
        match self {
            MixedStage::Dyn(stage) => Some(stage),
            MixedStage::Dummy(_) => None,
        }
    }

    fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<StageDynLang>> {
        match self {
            MixedStage::Dyn(stage) => Some(stage),
            MixedStage::Dummy(_) => None,
        }
    }
}

impl HasStageInfo<DummyLang> for MixedStage {
    fn try_stage_info(&self) -> Option<&StageInfo<DummyLang>> {
        match self {
            MixedStage::Dyn(_) => None,
            MixedStage::Dummy(stage) => Some(stage),
        }
    }

    fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<DummyLang>> {
        match self {
            MixedStage::Dyn(_) => None,
            MixedStage::Dummy(stage) => Some(stage),
        }
    }
}

#[test]
fn test_typed_call_reports_stage_mismatch() {
    let mut pipeline: Pipeline<MixedStage> = Pipeline::new();
    let dyn_stage = pipeline
        .add_stage()
        .stage(MixedStage::Dyn(StageInfo::default()))
        .new();
    let dummy_stage = pipeline
        .add_stage()
        .stage(MixedStage::Dummy(StageInfo::default()))
        .new();

    let func = pipeline.function().name("f").new();
    let staged = pipeline
        .staged_function::<StageDynLang>()
        .func(func)
        .stage(dyn_stage)
        .new()
        .unwrap();

    let spec = {
        let MixedStage::Dyn(stage) = pipeline.stage_mut(dyn_stage).unwrap() else {
            unreachable!();
        };
        let body = build_cross_stage_recursive_body(stage, func, dyn_stage);
        stage.specialize().f(staged).body(body).new().unwrap()
    };

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, dummy_stage);
    let err = interp
        .call_in_stage::<StageDynLang>(spec, &[2])
        .unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::TypedStageMismatch { frame_stage } if frame_stage == dummy_stage
        ),
        "unexpected error: {err:?}"
    );
}
