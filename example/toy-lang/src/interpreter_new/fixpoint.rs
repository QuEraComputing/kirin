use std::marker::PhantomData;

use kirin::prelude::{CompileStage, Dialect, Function, Pipeline};
use kirin_interpreter_new::{
    AbstractEnvStore, AbstractValue, FunctionFrame, OwnerSemantics, SimpleFixpointInterpreter,
    Summary, SummaryEffect,
};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

use super::run::{expect_function_return, resolve_function};
use super::{ConstProp, ToyCompletion, ToyError, ToyFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct FunctionOwner {
    stage: CompileStage,
    function: Function,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReturnSummary {
    value: ConstProp,
}

impl ReturnSummary {
    fn bottom() -> Self {
        Self {
            value: ConstProp::Bottom,
        }
    }
}

impl Summary for ReturnSummary {
    type Strategy = ();
    type Change = ();

    fn merge(
        &mut self,
        _phase: kirin_interpreter_new::FixpointPhase,
        candidate: Self,
        _strategy: &mut Self::Strategy,
    ) -> Option<Self::Change> {
        let joined = self.value.join(&candidate.value);
        if self.value == joined {
            None
        } else {
            self.value = joined;
            Some(())
        }
    }
}

struct FunctionConstPropSemantics<L> {
    args: Vec<ConstProp>,
    _marker: PhantomData<fn() -> L>,
}

impl<L> FunctionConstPropSemantics<L> {
    fn new(args: &[ConstProp]) -> Self {
        Self {
            args: args.to_vec(),
            _marker: PhantomData,
        }
    }
}

type FunctionFixpoint<'ir, L> = SimpleFixpointInterpreter<
    'ir,
    Stage,
    FunctionOwner,
    ToyFrame<L, ConstProp>,
    ToyCompletion<ConstProp>,
    ToyError,
    ReturnSummary,
    AbstractEnvStore<ConstProp>,
>;

impl<'ir, L>
    OwnerSemantics<
        FunctionFixpoint<'ir, L>,
        FunctionOwner,
        ReturnSummary,
        ToyFrame<L, ConstProp>,
        ToyCompletion<ConstProp>,
        ToyError,
    > for FunctionConstPropSemantics<L>
where
    L: Dialect,
    FunctionFrame<L, ConstProp>: Into<ToyFrame<L, ConstProp>>,
{
    fn bottom_summary(
        &mut self,
        _interp: &mut FunctionFixpoint<'ir, L>,
        _owner: &FunctionOwner,
    ) -> Result<ReturnSummary, ToyError> {
        Ok(ReturnSummary::bottom())
    }

    fn entry_frame(
        &mut self,
        _interp: &mut FunctionFixpoint<'ir, L>,
        owner: &FunctionOwner,
        _summary: &ReturnSummary,
    ) -> Result<ToyFrame<L, ConstProp>, ToyError> {
        Ok(
            FunctionFrame::<L, ConstProp>::new(owner.stage, owner.function, self.args.clone())
                .into(),
        )
    }

    fn complete_owner(
        &mut self,
        _interp: &mut FunctionFixpoint<'ir, L>,
        owner: FunctionOwner,
        completion: ToyCompletion<ConstProp>,
    ) -> Result<SummaryEffect<FunctionOwner, ReturnSummary>, ToyError> {
        Ok(SummaryEffect::Update {
            owner,
            candidate: ReturnSummary {
                value: expect_function_return(completion)?,
            },
        })
    }
}

pub fn analyze_source_constprop_fixpoint(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let stage =
        pipeline
            .stage_by_name("source")
            .ok_or(kirin_interpreter_new::InterpreterError::Custom(
                "missing source stage",
            ))?;
    let function = resolve_function(pipeline, function_name)?;
    let owner = FunctionOwner { stage, function };
    let mut interp: FunctionFixpoint<'_, HighLevel> =
        SimpleFixpointInterpreter::new(pipeline, AbstractEnvStore::new(), ());
    let mut semantics = FunctionConstPropSemantics::<HighLevel>::new(args);

    interp.solve(&mut semantics, owner)?;
    Ok(interp
        .summary(&owner)
        .map(|summary| summary.value.clone())
        .unwrap_or(ConstProp::Bottom))
}

pub fn analyze_lowered_constprop_fixpoint(
    pipeline: &Pipeline<Stage>,
    function_name: &str,
    args: &[ConstProp],
) -> Result<ConstProp, ToyError> {
    let stage = pipeline.stage_by_name("lowered").ok_or(
        kirin_interpreter_new::InterpreterError::Custom("missing lowered stage"),
    )?;
    let function = resolve_function(pipeline, function_name)?;
    let owner = FunctionOwner { stage, function };
    let mut interp: FunctionFixpoint<'_, LowLevel> =
        SimpleFixpointInterpreter::new(pipeline, AbstractEnvStore::new(), ());
    let mut semantics = FunctionConstPropSemantics::<LowLevel>::new(args);

    interp.solve(&mut semantics, owner)?;
    Ok(interp
        .summary(&owner)
        .map(|summary| summary.value.clone())
        .unwrap_or(ConstProp::Bottom))
}
