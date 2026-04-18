use std::collections::VecDeque;
use std::marker::PhantomData;

use kirin_interpreter::{AbstractValue, WideningStrategy};
use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageInfo, StageMeta, Symbol,
};
use rustc_hash::FxHashMap;

use crate::abstract_domain::BaseDomain;
use crate::core::Core;
use crate::env::{Env, Interpretable};
use crate::error::InterpreterError;

// ---------------------------------------------------------------------------
// AbstractInterp — worklist-based fixpoint interpreter
// ---------------------------------------------------------------------------

/// Worklist-based fixpoint abstract interpreter.
///
/// `type Cursor = ()` — abstract execution does not use a cursor stack;
/// the field exists so that `Core<V, E::Cursor>` is well-typed in dialect impls.
///
/// `type Effect = Core<V, ()>` — all effects are direct Core variants. Dialect
/// impls that return `E::Effect` will return `Core<V, ()>` here, which matches
/// the identity `Lift<Core<V, ()>>` and `Project<Core<V, ()>>` impls.
pub struct AbstractInterp<'ir, S: StageMeta, L: Dialect, V> {
    pipeline: &'ir Pipeline<S>,
    stage_id: CompileStage,
    /// Current SSA value state (reused across statement execution within a block).
    ssa_vals: FxHashMap<SSAValue, V>,
    /// Block entry states: block → abstract argument values at entry.
    block_in: FxHashMap<Block, Vec<V>>,
    /// Blocks pending analysis.
    worklist: VecDeque<Block>,
    /// Per-block visit counts for widening threshold.
    visit_counts: FxHashMap<Block, usize>,
    /// Accumulated abstract return/yield value (joined over all exits).
    pending_result: Option<V>,
    /// Widening strategy for fixpoint convergence.
    widening: WideningStrategy,
    _phantom: PhantomData<L>,
}

// -- Env --------------------------------------------------------------------

impl<'ir, S, L, V> Env for AbstractInterp<'ir, S, L, V>
where
    S: StageMeta,
    L: Dialect,
    V: Clone + AbstractValue,
{
    type Value = V;
    type Effect = Core<V, ()>;
    type Error = InterpreterError;

    fn current_stage(&self) -> CompileStage {
        self.stage_id
    }

    fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        self.ssa_vals
            .get(&ssa)
            .cloned()
            .ok_or(InterpreterError::UnboundValue(ssa))
    }

    fn write(&mut self, r: ResultValue, v: V) -> Result<(), InterpreterError> {
        self.ssa_vals.insert(SSAValue::from(r), v);
        Ok(())
    }

    fn write_ssa(&mut self, ssa: SSAValue, v: V) -> Result<(), InterpreterError> {
        self.ssa_vals.insert(ssa, v);
        Ok(())
    }
}

// -- BaseDomain -------------------------------------------------------------

impl<'ir, S, L, V> BaseDomain for AbstractInterp<'ir, S, L, V>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + AbstractValue,
{
    type Language = L;
    type Cursor = ();
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

// -- Constructor ------------------------------------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone + AbstractValue> AbstractInterp<'ir, S, L, V> {
    pub fn new(pipeline: &'ir Pipeline<S>, stage_id: CompileStage) -> Self {
        Self::with_widening(pipeline, stage_id, WideningStrategy::Delayed(3))
    }

    pub fn with_widening(
        pipeline: &'ir Pipeline<S>,
        stage_id: CompileStage,
        widening: WideningStrategy,
    ) -> Self {
        Self {
            pipeline,
            stage_id,
            ssa_vals: FxHashMap::default(),
            block_in: FxHashMap::default(),
            worklist: VecDeque::new(),
            visit_counts: FxHashMap::default(),
            pending_result: None,
            widening,
            _phantom: PhantomData,
        }
    }
}

// -- Fixpoint analysis ------------------------------------------------------

impl<'ir, S, L, V> AbstractInterp<'ir, S, L, V>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect + Interpretable<Self, DialectEffect = Core<V, ()>>,
    V: Clone + AbstractValue,
{
    /// Run fixpoint analysis from `entry_block` with initial argument values `args`.
    ///
    /// Returns the joined abstract return value once the worklist empties.
    pub fn analyze(
        &mut self,
        entry_block: Block,
        args: Vec<V>,
    ) -> Result<Option<V>, InterpreterError> {
        self.block_in.insert(entry_block, args);
        self.worklist.push_back(entry_block);

        while let Some(block) = self.worklist.pop_front() {
            self.run_block(block)?;
        }

        Ok(self.pending_result.take())
    }

    fn run_block(&mut self, block: Block) -> Result<(), InterpreterError> {
        // Phase 1: collect statement definitions and block argument SSA keys.
        // All borrows of `stage` end before Phase 2 mutates `ssa_vals`.
        let (ssa_keys, all_defs) = {
            let stage: &StageInfo<L> = self
                .pipeline
                .stage(self.stage_id)
                .and_then(|s| s.try_stage_info())
                .ok_or(InterpreterError::MissingEntry)?;

            let block_info = block.expect_info(stage);
            let ssa_keys: Vec<SSAValue> = block_info
                .arguments
                .iter()
                .map(|ba| SSAValue::from(*ba))
                .collect();

            let all_defs: Vec<L> = block
                .statements(stage)
                .chain(block.terminator(stage))
                .map(|s| s.definition(stage).clone())
                .collect();

            (ssa_keys, all_defs)
        };

        // Phase 2: bind block arguments.
        let entry_args = self.block_in.get(&block).cloned().unwrap_or_default();
        if ssa_keys.len() != entry_args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: ssa_keys.len(),
                got: entry_args.len(),
            });
        }
        for (ssa, val) in ssa_keys.into_iter().zip(entry_args.into_iter()) {
            self.ssa_vals.insert(ssa, val);
        }

        // Phase 3: execute statements.
        for def in all_defs {
            let effect = def.interpret(self)?;
            match effect {
                Core::Advance => {}
                Core::Jump(target, args) => {
                    self.propagate_to(target, args)?;
                    return Ok(());
                }
                Core::Return(v) | Core::Yield(v) => {
                    self.join_result(v);
                    return Ok(());
                }
                Core::Fork(b1, args1, b2, args2) => {
                    self.propagate_to(b1, args1)?;
                    self.propagate_to(b2, args2)?;
                    return Ok(());
                }
                Core::Push(_) | Core::Pop => {
                    return Err(InterpreterError::UnhandledEffect(
                        "Core::Push/Pop not supported in abstract interpreter; \
                         use structured control flow at an abstract-compatible stage"
                            .into(),
                    ));
                }
                Core::Call {
                    callee,
                    args,
                    results,
                    ..
                } => {
                    let result_val = self.analyze_call(callee, args)?;
                    if let Some(v) = result_val {
                        for r in &results {
                            self.ssa_vals.insert(SSAValue::from(*r), v.clone());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Join `v` into the accumulated return value.
    fn join_result(&mut self, v: V) {
        self.pending_result = Some(match self.pending_result.take() {
            None => v,
            Some(existing) => existing.join(&v),
        });
    }

    /// Propagate `args` to `block`, widening if necessary, and re-queue if the
    /// entry state changed.
    fn propagate_to(&mut self, block: Block, args: Vec<V>) -> Result<(), InterpreterError> {
        let widening = self.widening;
        let visit_count = *self.visit_counts.get(&block).unwrap_or(&0);

        let changed = if let Some(existing) = self.block_in.get(&block) {
            if existing.len() != args.len() {
                return Err(InterpreterError::ArityMismatch {
                    expected: existing.len(),
                    got: args.len(),
                });
            }
            let new_args: Vec<V> = existing
                .iter()
                .zip(args.iter())
                .map(|(e, a)| widening.merge(e, a, visit_count))
                .collect();
            let changed = new_args
                .iter()
                .zip(existing.iter())
                .any(|(n, o)| !n.is_subseteq(o));
            if changed {
                self.block_in.insert(block, new_args);
            }
            changed
        } else {
            self.block_in.insert(block, args);
            true
        };

        if changed {
            *self.visit_counts.entry(block).or_insert(0) += 1;
            if !self.worklist.contains(&block) {
                self.worklist.push_back(block);
            }
        }

        Ok(())
    }

    /// Inline analysis of a callee: create a fresh sub-interpreter and run it.
    fn analyze_call(
        &mut self,
        callee: SpecializedFunction,
        args: Vec<V>,
    ) -> Result<Option<V>, InterpreterError> {
        let entry_block = {
            let stage: &StageInfo<L> = self
                .pipeline
                .stage(self.stage_id)
                .and_then(|s| s.try_stage_info())
                .ok_or(InterpreterError::MissingEntry)?;

            let spec_info = callee
                .get_info(stage)
                .ok_or(InterpreterError::MissingEntry)?;
            let body_stmt = *spec_info.body();
            let definition = body_stmt.definition(stage).clone();
            let entry_region = *definition
                .regions()
                .next()
                .ok_or(InterpreterError::MissingEntry)?;
            entry_region
                .blocks(stage)
                .next()
                .ok_or(InterpreterError::MissingEntry)?
        };

        let mut sub = AbstractInterp::with_widening(self.pipeline, self.stage_id, self.widening);
        sub.analyze(entry_block, args)
    }
}
