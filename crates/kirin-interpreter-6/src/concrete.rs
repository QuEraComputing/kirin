use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageInfo, StageMeta, Symbol,
};

use crate::abstract_domain::BaseDomain;
use crate::core::Core;
use crate::cursor::{BlockCursor, Execute};
use crate::env::Env;
use crate::error::InterpreterError;
use crate::frame::Frame;
use crate::frame_stack::FrameStack;
use crate::lift::{Lift, Project};

// ---------------------------------------------------------------------------
// ConcreteDomain — cursor-stack execution on top of BaseDomain
// ---------------------------------------------------------------------------

/// Extended [`BaseDomain`] interface for concrete (cursor-stack) execution.
///
/// Adds `take_pending_yield` for extracting top-level return values from the
/// driver loop. All stage info lookup and function resolution are on
/// [`BaseDomain`].
///
/// `type Cursor` is the language's cursor coproduct type, e.g. `MyLangCursor<V>`.
/// It is `V`-parameterized only — no interpreter type appears in the cursor
/// definition. The `Execute<Self>` impl adds execution behavior.
///
/// Note: `Execute<Self>` is NOT a supertrait bound on `type Cursor` — adding
/// it would create an inductive cycle. The bound is added explicitly in
/// `ConcreteInterp::step` and `enter_function` instead.
pub trait ConcreteDomain: BaseDomain
where
    Self::Effect: Lift<Core<Self::Value, Self::Cursor>> + Project<Core<Self::Value, Self::Cursor>>,
{
    fn take_pending_yield(&mut self) -> Option<Self::Value>;
}

// ---------------------------------------------------------------------------
// ConcreteInterp — single-dialect concrete interpreter
// ---------------------------------------------------------------------------

/// Single-dialect concrete interpreter.
///
/// `S` — the stage container type (e.g. `StageInfo<L>` for single-dialect
///       pipelines, or a multi-stage `MyStages` enum).
/// `L` — the dialect (or composed language type).
/// `V` — value type.
/// `C` — cursor type, typically a language cursor coproduct `MyLangCursor<V>`.
/// `Eff` — effect type, defaults to `Core<V, C>`.
pub struct ConcreteInterp<'ir, S: StageMeta, L: Dialect, V: Clone, C, Eff = Core<V, C>> {
    pipeline: &'ir Pipeline<S>,
    stage_id: CompileStage,
    frames: FrameStack<V>,
    cursors: Vec<C>,
    pending_yield: Option<V>,
    _phantom: PhantomData<(L, Eff)>,
}

// -- Env --------------------------------------------------------------------

impl<'ir, S, L, V, C, Eff> Env for ConcreteInterp<'ir, S, L, V, C, Eff>
where
    S: StageMeta,
    L: Dialect,
    V: Clone,
    C: 'static,
    Eff: Lift<Core<V, C>> + Project<Core<V, C>> + 'static,
{
    type Value = V;
    type Effect = Eff;
    type Error = InterpreterError;

    fn current_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.stage_id)
    }

    fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        self.frames.read(ssa)
    }

    fn write(&mut self, r: ResultValue, v: V) -> Result<(), InterpreterError> {
        self.frames.write(r, v)
    }

    fn write_ssa(&mut self, ssa: SSAValue, v: V) -> Result<(), InterpreterError> {
        self.frames.write_ssa(ssa, v)
    }
}

// -- BaseDomain -------------------------------------------------------------

impl<'ir, S, L, V, C, Eff> BaseDomain for ConcreteInterp<'ir, S, L, V, C, Eff>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: 'static,
    Eff: Lift<Core<V, C>> + Project<Core<V, C>> + 'static,
{
    type Language = L;
    type Cursor = C;
    type StageContainer = S;

    fn stage_info_for<LD: Dialect>(&self, stage_id: CompileStage) -> Option<&StageInfo<LD>>
    where
        S: HasStageInfo<LD>,
    {
        self.pipeline.stage(stage_id)?.try_stage_info()
    }

    fn resolve_function(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, InterpreterError> {
        let stage_container = self
            .pipeline
            .stage(stage_id)
            .ok_or(InterpreterError::MissingEntry)?;
        let stage_info: &StageInfo<L> = stage_container
            .try_stage_info()
            .ok_or(InterpreterError::MissingEntry)?;
        let function = self
            .pipeline
            .resolve_function(stage_info, target)
            .ok_or(InterpreterError::MissingEntry)?;
        let staged_function = self
            .pipeline
            .function_info(function)
            .and_then(|info| info.staged_function(stage_id))
            .ok_or(InterpreterError::MissingEntry)?;
        staged_function
            .get_info(stage_info)
            .ok_or(InterpreterError::MissingEntry)?
            .unique_live_specialization()
            .map_err(|_| InterpreterError::UnhandledEffect("ambiguous specialization".into()))
    }
}

// -- ConcreteDomain ---------------------------------------------------------

impl<'ir, S, L, V, C, Eff> ConcreteDomain for ConcreteInterp<'ir, S, L, V, C, Eff>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: 'static,
    Eff: Lift<Core<V, C>> + Project<Core<V, C>> + 'static,
{
    fn take_pending_yield(&mut self) -> Option<V> {
        self.pending_yield.take()
    }
}

// -- Constructor ------------------------------------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone, C, Eff> ConcreteInterp<'ir, S, L, V, C, Eff> {
    pub fn new(pipeline: &'ir Pipeline<S>, stage_id: CompileStage) -> Self {
        Self {
            pipeline,
            stage_id,
            frames: FrameStack::new(),
            cursors: Vec::new(),
            pending_yield: None,
            _phantom: PhantomData,
        }
    }
}

impl<'ir, L: Dialect, V: Clone, C, Eff> ConcreteInterp<'ir, StageInfo<L>, L, V, C, Eff> {
    /// Convenience constructor for the common single-dialect case where the
    /// pipeline uses `StageInfo<L>` as its stage container.
    pub fn from_single_stage(
        pipeline: &'ir Pipeline<StageInfo<L>>,
        stage_id: CompileStage,
    ) -> Self {
        Self::new(pipeline, stage_id)
    }
}

// -- enter_function ---------------------------------------------------------

impl<'ir, S, L, V, C, Eff> ConcreteInterp<'ir, S, L, V, C, Eff>
where
    S: StageMeta,
    L: Dialect,
    V: Clone,
    C: Execute<Self> + 'static,
    Eff: Lift<Core<V, C>> + Project<Core<V, C>> + 'static,
{
    /// Push a call frame and a `BlockCursor<V, LD>` for the entry block.
    ///
    /// `LD` is the dialect of the callee (may differ from `L` in cross-stage calls).
    /// `C: Lift<BlockCursor<V, LD>>` injects the typed cursor into the coproduct.
    pub fn enter_function<LD: Dialect>(
        &mut self,
        callee: SpecializedFunction,
        entry_block: Block,
        args: &[V],
    ) -> Result<(), InterpreterError>
    where
        S: HasStageInfo<LD>,
        BlockCursor<V, LD>: Execute<Self> + 'static,
        C: Lift<BlockCursor<V, LD>>,
    {
        let cursor = BlockCursor::<V, LD>::new(entry_block, self.stage_id, args.to_vec());
        let frame = Frame::new(callee, self.stage_id, vec![]);
        self.frames.push(frame)?;
        self.cursors.push(C::lift(cursor));
        Ok(())
    }
}

// -- Driver loop ------------------------------------------------------------

impl<'ir, S, L, V, C, Eff> ConcreteInterp<'ir, S, L, V, C, Eff>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
    C: Execute<Self> + Lift<BlockCursor<V, L>> + 'static,
    Eff: Lift<Core<V, C>> + Project<Core<V, C>> + 'static,
{
    /// Execute one step. Returns `true` if work was done, `false` when finished.
    pub fn step(&mut self) -> Result<bool, InterpreterError> {
        let Some(mut cursor) = self.cursors.pop() else {
            return Ok(false);
        };

        let effect = cursor.execute(self)?;

        match Project::<Core<V, C>>::project(effect) {
            Ok(Core::Advance) => {
                self.cursors.push(cursor);
            }
            Ok(Core::Jump(..)) => {
                self.cursors.push(cursor);
            }
            Ok(Core::Push(new_cursor)) => {
                self.cursors.push(cursor);
                self.cursors.push(new_cursor);
            }
            Ok(Core::Pop) => {}
            Ok(Core::Yield(v)) => {
                self.pending_yield = Some(v);
            }
            Ok(Core::Return(v)) => {
                let frame = self.frames.pop().ok_or(InterpreterError::NoFrame)?;
                let caller_results = frame.caller_results().to_vec();
                if self.frames.is_empty() {
                    self.pending_yield = Some(v);
                } else {
                    for result in &caller_results {
                        self.frames.write(*result, v.clone())?;
                    }
                }
            }
            Ok(Core::Call {
                callee,
                stage,
                args,
                results,
            }) => {
                self.cursors.push(cursor);
                self.push_call_frame(callee, stage, args, results)?;
            }
            Ok(Core::Fork(..)) => {
                return Err(InterpreterError::UnhandledEffect(
                    "Core::Fork reached concrete driver; \
                     use AbstractInterp for nondeterministic branches"
                        .into(),
                ));
            }
            Err(_dialect_effect) => {
                return Err(InterpreterError::UnhandledEffect(
                    "non-Core effect reached driver loop; \
                     override step() or use a dialect-aware driver"
                        .into(),
                ));
            }
        }

        Ok(true)
    }

    /// Run until the cursor stack is empty or a top-level yield/return is produced.
    pub fn run(&mut self) -> Result<Option<V>, InterpreterError> {
        while self.step()? {}
        Ok(self.pending_yield.take())
    }

    fn push_call_frame(
        &mut self,
        callee: SpecializedFunction,
        _callee_stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<(), InterpreterError> {
        let stage_container = self
            .pipeline
            .stage(self.stage_id)
            .ok_or(InterpreterError::MissingEntry)?;
        let stage: &StageInfo<L> = stage_container
            .try_stage_info()
            .ok_or(InterpreterError::MissingEntry)?;
        let spec_info = callee
            .get_info(stage)
            .ok_or(InterpreterError::MissingEntry)?;
        let body_stmt = *spec_info.body();
        let definition = body_stmt.definition(stage);
        let entry_region = definition
            .regions()
            .next()
            .ok_or(InterpreterError::MissingEntry)?;
        let entry_block = entry_region
            .blocks(stage)
            .next()
            .ok_or(InterpreterError::MissingEntry)?;

        let cursor = BlockCursor::<V, L>::new(entry_block, self.stage_id, args);
        let frame = Frame::new(callee, self.stage_id, results);
        self.frames.push(frame)?;
        self.cursors.push(C::lift(cursor));
        Ok(())
    }
}
