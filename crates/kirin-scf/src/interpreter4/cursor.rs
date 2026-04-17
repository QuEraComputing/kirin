use kirin::prelude::{
    Block, CompileStage, Dialect, HasStageInfo, ResultValue, StageMeta, SupportsStageDispatch,
};
use kirin_interpreter::ProductValue;
use kirin_interpreter_4::concrete::{
    Action, Boxed, MakeBlockCursorAction, MultiStage, SingleStage,
};
use kirin_interpreter_4::cursor::BlockCursor;
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::execute::Execute;
use kirin_interpreter_4::lift::Lift;
use kirin_interpreter_4::traits::{Machine, PipelineAccess, ValueStore};

use crate::ForLoopValue;

// ---------------------------------------------------------------------------
// IfCursor — two-phase inline execution for scf.if
// ---------------------------------------------------------------------------

enum IfPhase<V> {
    PushBody {
        body: Block,
        results: Vec<ResultValue>,
    },
    CollectYield {
        results: Vec<ResultValue>,
    },
    Done(std::marker::PhantomData<V>),
}

pub struct IfCursor<V> {
    phase: IfPhase<V>,
    body_stage: CompileStage,
}

impl<V> IfCursor<V> {
    pub fn new(body: Block, results: Vec<ResultValue>, body_stage: CompileStage) -> Self {
        Self {
            phase: IfPhase::PushBody { body, results },
            body_stage,
        }
    }
}

// -- Execute<SingleStage> ---------------------------------------------------

impl<'ir, L, V, M, C> Execute<SingleStage<'ir, L, V, M, C>> for IfCursor<V>
where
    L: Dialect,
    <SingleStage<'ir, L, V, M, C> as PipelineAccess>::StageInfo: HasStageInfo<L>,
    V: Clone + ProductValue,
    M: Machine<Error = InterpreterError>,
    C: Lift<BlockCursor<V, L>>,
{
    fn execute(
        &mut self,
        interp: &mut SingleStage<'ir, L, V, M, C>,
    ) -> Result<Action<V, M::Effect, C>, InterpreterError> {
        match std::mem::replace(&mut self.phase, IfPhase::Done(std::marker::PhantomData)) {
            IfPhase::PushBody { body, results } => {
                let cursor = {
                    let stage = interp
                        .current_stage_info::<L>()
                        .ok_or(InterpreterError::MissingEntry)?;
                    BlockCursor::new(stage, body, vec![], vec![])
                };
                self.phase = IfPhase::CollectYield { results };
                Ok(Action::Push(Lift::lift(cursor)))
            }
            IfPhase::CollectYield { results } => {
                if let Some(product) = interp.take_pending_yield() {
                    write_product(interp, &results, product)?;
                }
                Ok(Action::Pop)
            }
            IfPhase::Done(_) => Err(InterpreterError::UnhandledEffect(
                "IfCursor executed after completion".into(),
            )),
        }
    }
}

// -- Execute<MultiStage> ----------------------------------------------------

impl<'ir, S, V, M> Execute<MultiStage<'ir, S, V, M>> for IfCursor<V>
where
    S: StageMeta + SupportsStageDispatch<MakeBlockCursorAction<'ir, S, V, M>, (), InterpreterError>,
    V: Clone + ProductValue,
    M: Machine<Error = InterpreterError>,
{
    fn execute(
        &mut self,
        interp: &mut MultiStage<'ir, S, V, M>,
    ) -> Result<Action<V, M::Effect, Boxed<'ir, MultiStage<'ir, S, V, M>>>, InterpreterError> {
        match std::mem::replace(&mut self.phase, IfPhase::Done(std::marker::PhantomData)) {
            IfPhase::PushBody { body, results } => {
                let cursor = interp.make_block_cursor(body, self.body_stage, vec![])?;
                self.phase = IfPhase::CollectYield { results };
                Ok(Action::Push(cursor))
            }
            IfPhase::CollectYield { results } => {
                if let Some(product) = interp.take_pending_yield() {
                    write_product(interp, &results, product)?;
                }
                Ok(Action::Pop)
            }
            IfPhase::Done(_) => Err(InterpreterError::UnhandledEffect(
                "IfCursor executed after completion".into(),
            )),
        }
    }
}

// -- Lift<IfCursor<V>> for Boxed<MultiStage<S>> -----------------------------
//
// Needed so that PushEffect<IfCursor<V>>: LiftInto<MultiStage::Effect> when
// StructuredControlFlow::interpret lifts the inner If result.
// IfCursor<V> is local to kirin-scf, so orphan rules allow this impl.

impl<'ir, S, V, M> Lift<IfCursor<V>> for Boxed<'ir, MultiStage<'ir, S, V, M>>
where
    S: StageMeta + SupportsStageDispatch<MakeBlockCursorAction<'ir, S, V, M>, (), InterpreterError>,
    V: Clone + ProductValue,
    M: Machine<Error = InterpreterError>,
    IfCursor<V>: Execute<MultiStage<'ir, S, V, M>> + 'ir,
{
    fn lift(from: IfCursor<V>) -> Self {
        Boxed(Box::new(from))
    }
}

// ---------------------------------------------------------------------------
// ForCursor — multi-phase inline execution for scf.for
// ---------------------------------------------------------------------------

enum ForPhase<V> {
    CheckAndPush {
        iv: V,
        end: V,
        step: V,
        carried: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
    },
    CollectAndStep {
        iv: V,
        end: V,
        step: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
    },
    Done(std::marker::PhantomData<V>),
}

pub struct ForCursor<V> {
    phase: ForPhase<V>,
    body_stage: CompileStage,
}

impl<V> ForCursor<V> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        iv: V,
        end: V,
        step: V,
        carried: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
        body_stage: CompileStage,
    ) -> Self {
        Self {
            phase: ForPhase::CheckAndPush {
                iv,
                end,
                step,
                carried,
                body,
                init_arg_count,
                results,
            },
            body_stage,
        }
    }
}

// -- Execute<SingleStage> ---------------------------------------------------

impl<'ir, L, V, M, C> Execute<SingleStage<'ir, L, V, M, C>> for ForCursor<V>
where
    L: Dialect,
    <SingleStage<'ir, L, V, M, C> as PipelineAccess>::StageInfo: HasStageInfo<L>,
    V: Clone + ProductValue + ForLoopValue,
    M: Machine<Error = InterpreterError>,
    C: Lift<BlockCursor<V, L>>,
{
    fn execute(
        &mut self,
        interp: &mut SingleStage<'ir, L, V, M, C>,
    ) -> Result<Action<V, M::Effect, C>, InterpreterError> {
        match std::mem::replace(&mut self.phase, ForPhase::Done(std::marker::PhantomData)) {
            ForPhase::CheckAndPush {
                iv,
                end,
                step,
                carried,
                body,
                init_arg_count,
                results,
            } => {
                if iv.loop_condition(&end) != Some(true) {
                    write_product(interp, &results, carried)?;
                    return Ok(Action::Pop);
                }

                let mut block_args = Vec::with_capacity(1 + init_arg_count);
                block_args.push(iv.clone());
                if let Some(product) = carried.as_product() {
                    block_args.extend(product.iter().cloned());
                } else if init_arg_count > 0 {
                    block_args.push(carried);
                }

                let cursor = {
                    let stage = interp
                        .current_stage_info::<L>()
                        .ok_or(InterpreterError::MissingEntry)?;
                    BlockCursor::new(stage, body, block_args, vec![])
                };
                self.phase = ForPhase::CollectAndStep {
                    iv,
                    end,
                    step,
                    body,
                    init_arg_count,
                    results,
                };
                Ok(Action::Push(Lift::lift(cursor)))
            }
            ForPhase::CollectAndStep {
                iv,
                end,
                step,
                body,
                init_arg_count,
                results,
            } => {
                let carried = interp.take_pending_yield().ok_or_else(|| {
                    InterpreterError::UnhandledEffect("scf.for body did not yield a value".into())
                })?;

                let next_iv = iv.loop_step(&step).ok_or_else(|| {
                    InterpreterError::UnhandledEffect(
                        "scf.for: induction variable overflow during loop step".into(),
                    )
                })?;

                self.phase = ForPhase::CheckAndPush {
                    iv: next_iv,
                    end,
                    step,
                    carried,
                    body,
                    init_arg_count,
                    results,
                };
                Ok(Action::Advance)
            }
            ForPhase::Done(_) => Err(InterpreterError::UnhandledEffect(
                "ForCursor executed after completion".into(),
            )),
        }
    }
}

// -- Execute<MultiStage> ----------------------------------------------------

impl<'ir, S, V, M> Execute<MultiStage<'ir, S, V, M>> for ForCursor<V>
where
    S: StageMeta + SupportsStageDispatch<MakeBlockCursorAction<'ir, S, V, M>, (), InterpreterError>,
    V: Clone + ProductValue + ForLoopValue,
    M: Machine<Error = InterpreterError>,
{
    fn execute(
        &mut self,
        interp: &mut MultiStage<'ir, S, V, M>,
    ) -> Result<Action<V, M::Effect, Boxed<'ir, MultiStage<'ir, S, V, M>>>, InterpreterError> {
        match std::mem::replace(&mut self.phase, ForPhase::Done(std::marker::PhantomData)) {
            ForPhase::CheckAndPush {
                iv,
                end,
                step,
                carried,
                body,
                init_arg_count,
                results,
            } => {
                if iv.loop_condition(&end) != Some(true) {
                    write_product(interp, &results, carried)?;
                    return Ok(Action::Pop);
                }

                let mut block_args = Vec::with_capacity(1 + init_arg_count);
                block_args.push(iv.clone());
                if let Some(product) = carried.as_product() {
                    block_args.extend(product.iter().cloned());
                } else if init_arg_count > 0 {
                    block_args.push(carried);
                }

                let cursor = interp.make_block_cursor(body, self.body_stage, block_args)?;
                self.phase = ForPhase::CollectAndStep {
                    iv,
                    end,
                    step,
                    body,
                    init_arg_count,
                    results,
                };
                Ok(Action::Push(cursor))
            }
            ForPhase::CollectAndStep {
                iv,
                end,
                step,
                body,
                init_arg_count,
                results,
            } => {
                let carried = interp.take_pending_yield().ok_or_else(|| {
                    InterpreterError::UnhandledEffect("scf.for body did not yield a value".into())
                })?;

                let next_iv = iv.loop_step(&step).ok_or_else(|| {
                    InterpreterError::UnhandledEffect(
                        "scf.for: induction variable overflow during loop step".into(),
                    )
                })?;

                self.phase = ForPhase::CheckAndPush {
                    iv: next_iv,
                    end,
                    step,
                    carried,
                    body,
                    init_arg_count,
                    results,
                };
                Ok(Action::Advance)
            }
            ForPhase::Done(_) => Err(InterpreterError::UnhandledEffect(
                "ForCursor executed after completion".into(),
            )),
        }
    }
}

// -- Lift<ForCursor<V>> for Boxed<MultiStage<S>> ----------------------------

impl<'ir, S, V, M> Lift<ForCursor<V>> for Boxed<'ir, MultiStage<'ir, S, V, M>>
where
    S: StageMeta + SupportsStageDispatch<MakeBlockCursorAction<'ir, S, V, M>, (), InterpreterError>,
    V: Clone + ProductValue + ForLoopValue,
    M: Machine<Error = InterpreterError>,
    ForCursor<V>: Execute<MultiStage<'ir, S, V, M>> + 'ir,
{
    fn lift(from: ForCursor<V>) -> Self {
        Boxed(Box::new(from))
    }
}

// ---------------------------------------------------------------------------
// Helper: write product value to result slots
// ---------------------------------------------------------------------------

fn write_product<I>(
    interp: &mut I,
    results: &[ResultValue],
    product: I::Value,
) -> Result<(), InterpreterError>
where
    I: ValueStore<Error = InterpreterError>,
    I::Value: ProductValue,
{
    if results.is_empty() {
        return Ok(());
    }
    if results.len() == 1 {
        interp.write(results[0], product)?;
    } else if let Some(components) = product.as_product() {
        for (result, value) in results.iter().zip(components.iter()) {
            interp.write(*result, value.clone())?;
        }
    } else {
        interp.write(results[0], product)?;
    }
    Ok(())
}
