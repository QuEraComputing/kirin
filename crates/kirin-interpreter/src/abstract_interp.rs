use std::collections::VecDeque;
use std::marker::PhantomData;

use fxhash::FxHashMap;
use kirin_ir::{
    Block, CompileStage, CompileStageInfo, Dialect, GetInfo, HasStageInfo, Pipeline, ResultValue,
    SSAValue, SpecializedFunction, StageInfo,
};

use crate::result::AnalysisResult;
use crate::widening::WideningStrategy;
use crate::{AbstractControl, AbstractValue, Frame, Interpretable, Interpreter, InterpreterError};

/// Per-function fixpoint state stored as frame extra data.
///
/// Uses a single flat map for all SSA values. Only block argument SSA
/// values need join/widen at edges — everything else is write-once.
#[derive(Debug)]
pub(crate) struct FixpointState<V> {
    current_block: Option<Block>,
    /// Single flat map for ALL SSA values (block args + statement results).
    values: FxHashMap<SSAValue, V>,
    worklist: VecDeque<Block>,
    /// Per-block argument SSA value IDs. Key presence = block visited.
    block_args: FxHashMap<Block, Vec<SSAValue>>,
}

impl<V> Default for FixpointState<V> {
    fn default() -> Self {
        Self {
            current_block: None,
            values: FxHashMap::default(),
            worklist: VecDeque::new(),
            block_args: FxHashMap::default(),
        }
    }
}

/// Worklist-based abstract interpreter for fixpoint computation.
///
/// Unlike [`crate::StackInterpreter`] which follows a single concrete execution
/// path, `AbstractInterpreter` explores all reachable paths by joining abstract
/// states at block entry points and iterating until a fixpoint is reached.
///
/// Widening is applied at join points to guarantee termination for infinite
/// abstract domains.
pub struct AbstractInterpreter<'ir, V, S, E = crate::InterpError, G = ()>
where
    S: CompileStageInfo,
{
    pipeline: &'ir Pipeline<S>,
    active_stage: CompileStage,
    global: G,
    frames: Vec<Frame<V, FixpointState<V>>>,
    widening_strategy: WideningStrategy,
    max_iterations: usize,
    narrowing_iterations: usize,
    summaries: FxHashMap<SpecializedFunction, AnalysisResult<V>>,
    max_depth: Option<usize>,
    _error: PhantomData<E>,
}

// -- Constructors -----------------------------------------------------------

impl<'ir, V, S, E> AbstractInterpreter<'ir, V, S, E, ()>
where
    S: CompileStageInfo,
{
    pub fn new(pipeline: &'ir Pipeline<S>, active_stage: CompileStage) -> Self {
        Self {
            pipeline,
            active_stage,
            global: (),
            widening_strategy: WideningStrategy::AllJoins,
            max_iterations: 1000,
            narrowing_iterations: 3,
            summaries: FxHashMap::default(),
            frames: Vec::new(),
            max_depth: None,
            _error: PhantomData,
        }
    }

    /// Attach global state, transforming `G` from `()` to the provided type.
    pub fn with_global<G>(self, global: G) -> AbstractInterpreter<'ir, V, S, E, G> {
        AbstractInterpreter {
            pipeline: self.pipeline,
            active_stage: self.active_stage,
            global,
            widening_strategy: self.widening_strategy,
            max_iterations: self.max_iterations,
            narrowing_iterations: self.narrowing_iterations,
            summaries: self.summaries,
            frames: self.frames,
            max_depth: self.max_depth,
            _error: PhantomData,
        }
    }
}

// -- Builder methods --------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    S: CompileStageInfo,
{
    pub fn with_widening(mut self, strategy: WideningStrategy) -> Self {
        self.widening_strategy = strategy;
        self
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_narrowing_iterations(mut self, n: usize) -> Self {
        self.narrowing_iterations = n;
        self
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }
}

// -- Accessors --------------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    S: CompileStageInfo,
{
    pub fn pipeline(&self) -> &'ir Pipeline<S> {
        self.pipeline
    }

    pub fn global(&self) -> &G {
        &self.global
    }

    pub fn global_mut(&mut self) -> &mut G {
        &mut self.global
    }

    /// Look up a cached function summary.
    pub fn summary(&self, callee: SpecializedFunction) -> Option<&AnalysisResult<V>> {
        self.summaries.get(&callee)
    }
}

// -- Frame helpers ----------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    E: InterpreterError,
    S: CompileStageInfo,
{
    fn current_fixpoint(&self) -> Result<&FixpointState<V>, E> {
        self.frames
            .last()
            .map(|f| f.extra())
            .ok_or_else(E::no_frame)
    }

    fn current_fixpoint_mut(&mut self) -> Result<&mut FixpointState<V>, E> {
        self.frames
            .last_mut()
            .map(|f| f.extra_mut())
            .ok_or_else(E::no_frame)
    }
}

// -- Interpreter trait impl -------------------------------------------------

impl<'ir, V, S, E, G> Interpreter for AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone,
    E: InterpreterError,
    S: CompileStageInfo,
{
    type Value = V;
    type Error = E;
    type Control = AbstractControl<V>;

    fn read_ref(&self, value: SSAValue) -> Result<&V, E> {
        let fp = self.current_fixpoint()?;
        fp.values.get(&value).ok_or_else(|| E::unbound_value(value))
    }

    fn write(&mut self, result: ResultValue, value: V) -> Result<(), E> {
        self.current_fixpoint_mut()?
            .values
            .insert(result.into(), value);
        Ok(())
    }
}

// -- Execution engine -------------------------------------------------------

impl<'ir, V, S, E, G> AbstractInterpreter<'ir, V, S, E, G>
where
    V: AbstractValue + Clone,
    E: InterpreterError,
    S: CompileStageInfo,
{
    /// Resolve the entry block of a specialized function.
    fn resolve_entry_block<L>(&self, callee: SpecializedFunction) -> Option<Block>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        let stage = self.resolve_stage::<L>();
        let spec = callee.expect_info(stage);
        let body_stmt = *spec.body();
        let region = body_stmt.regions::<L>(stage).next()?;
        region.blocks(stage).next()
    }

    /// Analyze a function, returning its [`AnalysisResult`].
    ///
    /// Results are cached in a context-insensitive summary table keyed by
    /// `callee`. Recursive calls and depth-limit violations return
    /// appropriate errors.
    ///
    /// Dialect `Interpretable` impls for call statements should resolve the
    /// callee to a [`SpecializedFunction`], call this method, bind the return
    /// value, and return [`AbstractControl::Continue`].
    pub fn analyze<L>(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<AnalysisResult<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<Self>,
    {
        // Check summary cache
        if let Some(cached) = self.summaries.get(&callee) {
            return Ok(cached.clone());
        }

        // Check recursion
        if self.frames.iter().any(|f| f.callee() == callee) {
            return Err(E::unexpected_control("recursive call detected"));
        }

        // Check depth limit
        if let Some(max) = self.max_depth {
            if self.frames.len() >= max {
                return Err(E::max_depth_exceeded());
            }
        }

        // Resolve entry block
        let entry = self
            .resolve_entry_block::<L>(callee)
            .ok_or_else(E::missing_entry)?;

        // Push frame with fresh fixpoint state
        self.frames
            .push(Frame::new(callee, FixpointState::default()));

        // Run fixpoint analysis on the callee
        let result = self.run_forward::<L>(entry, args);

        // Pop frame
        self.frames.pop().expect("frame stack underflow");

        // Cache and return
        let result = result?;
        self.summaries.insert(callee, result.clone());
        Ok(result)
    }

    /// Run forward abstract interpretation starting from `entry` block with
    /// `initial_args` bound to the block's arguments.
    ///
    /// Returns an [`AnalysisResult`] containing all SSA values and the joined
    /// return value.
    pub fn run_forward<L>(
        &mut self,
        entry: Block,
        initial_args: &[V],
    ) -> Result<AnalysisResult<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<Self>,
    {
        // 1. Seed entry block
        {
            let stage = self.resolve_stage::<L>();
            let block_info = entry.expect_info(stage);
            let arg_ssas: Vec<SSAValue> = block_info
                .arguments
                .iter()
                .map(|ba| SSAValue::from(*ba))
                .collect();
            let fp = self.current_fixpoint_mut()?;
            for (ssa, val) in arg_ssas.iter().zip(initial_args.iter()) {
                fp.values.insert(*ssa, val.clone());
            }
            fp.block_args.insert(entry, arg_ssas);
            fp.worklist.push_back(entry);
        }

        let mut return_value: Option<V> = None;
        let mut iterations = 0;

        // 2. Widening fixpoint loop
        loop {
            let block = {
                let fp = self.current_fixpoint_mut()?;
                fp.worklist.pop_front()
            };
            let Some(block) = block else { break };

            iterations += 1;
            if iterations > self.max_iterations {
                break;
            }

            // Set current block for interpretation
            {
                let fp = self.current_fixpoint_mut()?;
                fp.current_block = Some(block);
            }

            // Interpret the block
            let control = self.interpret_block::<L>(block)?;

            // Handle control flow
            match control {
                AbstractControl::Jump(target, ref args) => {
                    if self.propagate_block_args::<L>(target, args, false) {
                        let fp = self.current_fixpoint_mut()?;
                        if !fp.worklist.contains(&target) {
                            fp.worklist.push_back(target);
                        }
                    }
                }
                AbstractControl::Fork(ref targets) => {
                    for (target, args) in targets {
                        if self.propagate_block_args::<L>(*target, args, false) {
                            let fp = self.current_fixpoint_mut()?;
                            if !fp.worklist.contains(target) {
                                fp.worklist.push_back(*target);
                            }
                        }
                    }
                }
                AbstractControl::Return(ref v) => {
                    return_value = Some(match return_value {
                        Some(existing) => existing.join(v),
                        None => v.clone(),
                    });
                }
                AbstractControl::Continue | AbstractControl::Call { .. } => {}
            }
        }

        // 3. Narrowing phase
        if self.narrowing_iterations > 0 {
            let blocks: Vec<Block> = self
                .current_fixpoint()?
                .block_args
                .keys()
                .copied()
                .collect();
            for _ in 0..self.narrowing_iterations {
                let mut changed = false;
                for &block in &blocks {
                    {
                        let fp = self.current_fixpoint_mut()?;
                        fp.current_block = Some(block);
                    }

                    let control = self.interpret_block::<L>(block)?;

                    match control {
                        AbstractControl::Jump(target, ref args) => {
                            if self.propagate_block_args::<L>(target, args, true) {
                                changed = true;
                            }
                        }
                        AbstractControl::Fork(ref targets) => {
                            for (target, args) in targets {
                                if self.propagate_block_args::<L>(*target, args, true) {
                                    changed = true;
                                }
                            }
                        }
                        AbstractControl::Return(ref v) => {
                            return_value = Some(match return_value {
                                Some(existing) => existing.narrow(v),
                                None => v.clone(),
                            });
                        }
                        _ => {}
                    }
                }
                if !changed {
                    break;
                }
            }
        }

        let fp = self.current_fixpoint()?;
        Ok(AnalysisResult::new(
            fp.values.clone(),
            fp.block_args.clone(),
            return_value,
        ))
    }

    // -- Internal helpers ---------------------------------------------------

    fn resolve_stage<L>(&self) -> &'ir StageInfo<L>
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        self.pipeline
            .stage(self.active_stage)
            .and_then(|s| s.try_stage_info())
            .expect("active stage does not contain StageInfo for this dialect")
    }

    /// Interpret all statements in a block sequentially, returning the
    /// final control action from the terminator.
    fn interpret_block<L>(&mut self, block: Block) -> Result<AbstractControl<V>, E>
    where
        S: HasStageInfo<L>,
        L: Dialect + Interpretable<Self>,
    {
        // Collect statement IDs and terminator up front (cheap Copy)
        // to avoid holding a borrow on stage across interpret calls.
        let (stmts, terminator) = {
            let stage = self.resolve_stage::<L>();
            let stmts: Vec<_> = block.statements(stage).collect();
            let terminator = block.terminator(stage);
            (stmts, terminator)
        };

        // Interpret each statement
        for stmt in stmts {
            let control = {
                let stage = self.resolve_stage::<L>();
                let def: &L = stmt.definition(stage);
                def.interpret(self)?
            };
            match control {
                AbstractControl::Continue => {}
                AbstractControl::Call {
                    callee,
                    args,
                    result,
                } => {
                    let analysis = self.analyze::<L>(callee, &args)?;
                    let return_val = analysis
                        .return_value()
                        .cloned()
                        .ok_or_else(E::missing_entry)?;
                    self.write(result, return_val)?;
                }
                other => return Ok(other),
            }
        }

        // Interpret the terminator
        if let Some(term) = terminator {
            let control = {
                let stage = self.resolve_stage::<L>();
                let def: &L = term.definition(stage);
                def.interpret(self)?
            };
            Ok(control)
        } else {
            Ok(AbstractControl::Return(
                self.current_fixpoint()?
                    .values
                    .values()
                    .next()
                    .cloned()
                    .ok_or_else(E::no_frame)?,
            ))
        }
    }

    /// Propagate block argument values to the target block. Only block
    /// argument SSA values are joined/widened — all other SSA values in
    /// the flat map are write-once and shared across all paths.
    ///
    /// Returns `true` if the target's block arg state changed (or first visit).
    fn propagate_block_args<L>(&mut self, target: Block, args: &[V], narrowing: bool) -> bool
    where
        S: HasStageInfo<L>,
        L: Dialect,
    {
        // Look up target block's argument SSA values from stage info
        let target_arg_ssas: Vec<SSAValue> = {
            let stage: &StageInfo<L> = self.resolve_stage::<L>();
            let block_info = target.expect_info(stage);
            block_info
                .arguments
                .iter()
                .map(|ba| SSAValue::from(*ba))
                .collect()
        };

        let fp = self.frames.last_mut().expect("no active frame");
        let fp_state = fp.extra_mut();

        let first_visit = !fp_state.block_args.contains_key(&target);

        if first_visit {
            // First visit: write arg values to flat map, record block arg SSA IDs
            for (ssa, val) in target_arg_ssas.iter().zip(args.iter()) {
                fp_state.values.insert(*ssa, val.clone());
            }
            fp_state.block_args.insert(target, target_arg_ssas);
            true
        } else {
            // Subsequent visit: widen/narrow only block arg values
            let mut changed = false;
            for (ssa, new_val) in target_arg_ssas.iter().zip(args.iter()) {
                let merged = match fp_state.values.get(ssa) {
                    Some(old_val) => {
                        if narrowing {
                            old_val.narrow(new_val)
                        } else {
                            match self.widening_strategy {
                                WideningStrategy::AllJoins => old_val.widen(new_val),
                            }
                        }
                    }
                    None => new_val.clone(),
                };
                if let Some(old_val) = fp_state.values.get(ssa) {
                    if !merged.is_subseteq(old_val) || !old_val.is_subseteq(&merged) {
                        changed = true;
                    }
                } else {
                    changed = true;
                }
                fp_state.values.insert(*ssa, merged);
            }
            changed
        }
    }
}
