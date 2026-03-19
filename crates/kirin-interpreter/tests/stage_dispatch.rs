use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::{FunctionBody, Return};
use kirin_interpreter::{
    Continuation, InterpreterError, StackInterpreter, StageAccess, StageResolutionError,
};
use kirin_ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = T, crate = kirin_ir)]
struct StageCall<T: CompileTimeValue> {
    target: Function,
    callee_stage: CompileStage,
    args: Vec<SSAValue>,
    result: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> StageCall<T> {
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

impl<'ir, I, T> kirin_interpreter::Interpretable<'ir, I> for StageCall<T>
where
    I: kirin_interpreter::Interpreter<'ir>,
    I::Error: From<InterpreterError>,
    I::Value: Clone,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: kirin_interpreter::Interpretable<'ir, I> + 'ir,
    {
        let target_stage = self.target_stage();
        let stage = interp.resolve_stage_info::<L>(target_stage)?;

        let function_info = interp.pipeline().function_info(self.target()).ok_or(
            InterpreterError::StageResolution {
                stage: target_stage,
                kind: StageResolutionError::MissingFunction {
                    function: self.target(),
                },
            },
        )?;
        let staged_function = function_info
            .staged_functions()
            .get(&target_stage)
            .copied()
            .ok_or(InterpreterError::StageResolution {
                stage: target_stage,
                kind: StageResolutionError::MissingFunction {
                    function: self.target(),
                },
            })?;
        let staged_info =
            staged_function
                .get_info(stage)
                .ok_or(InterpreterError::StageResolution {
                    stage: target_stage,
                    kind: StageResolutionError::MissingFunction {
                        function: self.target(),
                    },
                })?;

        let mut live = staged_info
            .specializations()
            .iter()
            .filter(|spec| !spec.is_invalidated())
            .map(|spec| spec.id());
        let callee = match (live.next(), live.next()) {
            (None, _) => {
                return Err(InterpreterError::StageResolution {
                    stage: target_stage,
                    kind: StageResolutionError::NoSpecialization { staged_function },
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
                return Err(InterpreterError::StageResolution {
                    stage: target_stage,
                    kind: StageResolutionError::AmbiguousSpecialization {
                        staged_function,
                        count,
                    },
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[wraps]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
enum StageDynLang {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    StageCall(StageCall<ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[wraps]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
enum FunctionCallLang {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    Call(kirin_function::Call<ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

fn specialize_return_const(
    stage: &mut BuilderStageInfo<FunctionCallLang>,
    staged_function: StagedFunction,
    value: i64,
    with_arg: bool,
) -> SpecializedFunction {
    let c = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(value));
    let ret = Return::<ArithType>::new(stage, c.result);
    let block = stage.block().stmt(c).terminator(ret).new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    if with_arg {
        stage
            .specialize(
                staged_function,
                Some(Signature::new(vec![ArithType::I64], ArithType::I64, ())),
                body,
                None,
            )
            .unwrap()
    } else {
        stage.specialize(staged_function, None, body, None).unwrap()
    }
}

fn build_cross_stage_recursive_body(
    stage: &mut BuilderStageInfo<StageDynLang>,
    target_func: Function,
    target_stage: CompileStage,
) -> Statement {
    use kirin_ir::query::ParentInfo;

    let entry = stage.block().argument(ArithType::I64).new();
    let call_block = stage.block().argument(ArithType::I64).new();
    let exit_block = stage.block().new();

    let n: SSAValue = stage.block_arena()[entry].arguments[0].into();
    let call_arg: SSAValue = stage.block_arena()[call_block].arguments[0].into();

    let c0 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(0));
    let ret0 = Return::<ArithType>::new(stage, c0.result);
    {
        let stmts: Vec<Statement> = vec![c0.into()];
        for stmt in &stmts {
            *stage.statement_arena_mut()[*stmt].get_parent_mut() =
                Some(StatementParent::Block(exit_block));
        }
        let linked = stage.link_statements(&stmts);
        let ret_stmt: Statement = ret0.into();
        *stage.statement_arena_mut()[ret_stmt].get_parent_mut() =
            Some(StatementParent::Block(exit_block));
        let exit_info = stage.block_arena_mut().get_mut(exit_block).unwrap();
        exit_info.statements = linked;
        exit_info.terminator = Some(ret_stmt);
    }

    let call = StageCall::<ArithType>::new(stage, target_func, target_stage, vec![call_arg]);
    let ret = Return::<ArithType>::new(stage, call.result);
    {
        let call_stmt: Statement = call.into();
        *stage.statement_arena_mut()[call_stmt].get_parent_mut() =
            Some(StatementParent::Block(call_block));
        let linked = stage.link_statements(&[call_stmt]);
        let ret_stmt: Statement = ret.into();
        *stage.statement_arena_mut()[ret_stmt].get_parent_mut() =
            Some(StatementParent::Block(call_block));
        let call_info = stage.block_arena_mut().get_mut(call_block).unwrap();
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
            *stage.statement_arena_mut()[*stmt].get_parent_mut() =
                Some(StatementParent::Block(entry));
        }
        let linked = stage.link_statements(&stmts);
        let cond_stmt: Statement = cond.into();
        *stage.statement_arena_mut()[cond_stmt].get_parent_mut() =
            Some(StatementParent::Block(entry));
        let entry_info = stage.block_arena_mut().get_mut(entry).unwrap();
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
    let stage_a = pipeline.add_stage(StageInfo::default(), None::<&str>);
    let stage_b = pipeline.add_stage(StageInfo::default(), None::<&str>);

    let func = pipeline.function(Some("rec")).unwrap();
    let staged_a = pipeline
        .staged_function::<StageDynLang>(func, stage_a, None, None, None)
        .unwrap();
    let staged_b = pipeline
        .staged_function::<StageDynLang>(func, stage_b, None, None, None)
        .unwrap();

    let spec_a = pipeline.stage_mut(stage_a).unwrap().with_builder(|b| {
        let body = build_cross_stage_recursive_body(b, func, stage_b);
        b.specialize(staged_a, None, body, None).unwrap()
    });
    pipeline.stage_mut(stage_b).unwrap().with_builder(|b| {
        let body = build_cross_stage_recursive_body(b, func, stage_a);
        b.specialize(staged_b, None, body, None).unwrap();
    });

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_a);
    let result = interp.call(spec_a, stage_a, &[6]).unwrap();
    assert_eq!(result, 0);
}

fn build_caller_with_function_call(
    stage: &mut BuilderStageInfo<FunctionCallLang>,
    target: &str,
) -> Statement {
    let target_symbol = stage.symbol_table_mut().intern(target.to_string());
    let call = kirin_function::Call::<ArithType>::new(stage, target_symbol, vec![]);
    let ret = Return::<ArithType>::new(stage, call.res);
    let block = stage.block().stmt(call).terminator(ret).new();
    let region = stage.region().add_block(block).new();
    FunctionBody::<ArithType>::new(stage, region).into()
}

#[test]
fn test_function_call_missing_stage_mapping_error() {
    let mut pipeline: Pipeline<StageInfo<FunctionCallLang>> = Pipeline::new();
    let stage_a = pipeline.add_stage(StageInfo::default(), None::<&str>);
    let stage_b = pipeline.add_stage(StageInfo::default(), None::<&str>);

    let callee_func = pipeline.function(Some("callee")).unwrap();
    let caller_func = pipeline.function(Some("caller")).unwrap();

    let callee_staged_a = pipeline
        .staged_function::<FunctionCallLang>(callee_func, stage_a, None, None, None)
        .unwrap();
    let caller_staged_b = pipeline
        .staged_function::<FunctionCallLang>(caller_func, stage_b, None, None, None)
        .unwrap();

    {
        pipeline.stage_mut(stage_a).unwrap().with_builder(|b| {
            specialize_return_const(b, callee_staged_a, 7, false);
        });
    }

    let caller_spec = pipeline.stage_mut(stage_b).unwrap().with_builder(|b| {
        let body = build_caller_with_function_call(b, "callee");
        b.specialize(caller_staged_b, None, body, None).unwrap()
    });

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_b);
    let err = interp.call(caller_spec, stage_b, &[]).unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::StageResolution {
                stage,
                kind: StageResolutionError::MissingFunction { function },
            } if function == callee_func && stage == stage_b
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn test_function_call_no_specialization_error() {
    let mut pipeline: Pipeline<StageInfo<FunctionCallLang>> = Pipeline::new();
    let stage = pipeline.add_stage(StageInfo::default(), None::<&str>);

    let callee_func = pipeline.function(Some("callee")).unwrap();
    let caller_func = pipeline.function(Some("caller")).unwrap();

    let _callee_staged = pipeline
        .staged_function::<FunctionCallLang>(callee_func, stage, None, None, None)
        .unwrap();
    let caller_staged = pipeline
        .staged_function::<FunctionCallLang>(caller_func, stage, None, None, None)
        .unwrap();

    let caller_spec = pipeline.stage_mut(stage).unwrap().with_builder(|b| {
        let body = build_caller_with_function_call(b, "callee");
        b.specialize(caller_staged, None, body, None).unwrap()
    });

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage);
    let err = interp.call(caller_spec, stage, &[]).unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::StageResolution {
                stage: err_stage,
                kind: StageResolutionError::NoSpecialization { .. },
            } if err_stage == stage
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn test_function_call_ambiguous_specialization_error() {
    let mut pipeline: Pipeline<StageInfo<FunctionCallLang>> = Pipeline::new();
    let stage = pipeline.add_stage(StageInfo::default(), None::<&str>);

    let callee_func = pipeline.function(Some("callee")).unwrap();
    let caller_func = pipeline.function(Some("caller")).unwrap();

    let callee_staged = pipeline
        .staged_function::<FunctionCallLang>(callee_func, stage, None, None, None)
        .unwrap();
    let caller_staged = pipeline
        .staged_function::<FunctionCallLang>(caller_func, stage, None, None, None)
        .unwrap();

    {
        pipeline.stage_mut(stage).unwrap().with_builder(|b| {
            specialize_return_const(b, callee_staged, 3, false);
            specialize_return_const(b, callee_staged, 5, true);
        });
    }

    let caller_spec = pipeline.stage_mut(stage).unwrap().with_builder(|b| {
        let body = build_caller_with_function_call(b, "callee");
        b.specialize(caller_staged, None, body, None).unwrap()
    });

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage);
    let err = interp.call(caller_spec, stage, &[]).unwrap_err();
    assert!(
        matches!(
            err,
            InterpreterError::StageResolution {
                stage: err_stage,
                kind: StageResolutionError::AmbiguousSpecialization { count, .. },
            } if err_stage == stage && count == 2
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn test_function_call_unique_specialization_success() {
    let mut pipeline: Pipeline<StageInfo<FunctionCallLang>> = Pipeline::new();
    let stage = pipeline.add_stage(StageInfo::default(), None::<&str>);

    let callee_func = pipeline.function(Some("callee")).unwrap();
    let caller_func = pipeline.function(Some("caller")).unwrap();

    let callee_staged = pipeline
        .staged_function::<FunctionCallLang>(callee_func, stage, None, None, None)
        .unwrap();
    let caller_staged = pipeline
        .staged_function::<FunctionCallLang>(caller_func, stage, None, None, None)
        .unwrap();

    {
        pipeline.stage_mut(stage).unwrap().with_builder(|b| {
            specialize_return_const(b, callee_staged, 11, false);
        });
    }

    let caller_spec = pipeline.stage_mut(stage).unwrap().with_builder(|b| {
        let body = build_caller_with_function_call(b, "callee");
        b.specialize(caller_staged, None, body, None).unwrap()
    });

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage);
    let result = interp.call(caller_spec, stage, &[]).unwrap();
    assert_eq!(result, 11);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[wraps]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
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
#[should_panic(expected = "active stage does not contain StageInfo for this dialect")]
fn test_typed_call_reports_stage_mismatch() {
    let mut pipeline: Pipeline<MixedStage> = Pipeline::new();
    let dyn_stage = pipeline.add_stage(MixedStage::Dyn(StageInfo::default()), None::<&str>);
    let dummy_stage = pipeline.add_stage(MixedStage::Dummy(StageInfo::default()), None::<&str>);

    let func = pipeline.function(Some("f")).unwrap();
    let staged = pipeline
        .staged_function::<StageDynLang>(func, dyn_stage, None, None, None)
        .unwrap();

    let spec = {
        let MixedStage::Dyn(stage) = pipeline.stage_mut(dyn_stage).unwrap() else {
            unreachable!();
        };
        stage.with_builder(|b| {
            let body = build_cross_stage_recursive_body(b, func, dyn_stage);
            b.specialize(staged, None, body, None).unwrap()
        })
    };

    let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, dummy_stage);
    // `in_stage` panics when the active stage does not contain the requested dialect's StageInfo.
    let _ = interp.in_stage::<StageDynLang>().call(spec, &[2]);
}
