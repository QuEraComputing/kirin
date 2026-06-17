//! Backward liveness solver: generic use/def transfer, a CFG worklist with
//! precise block-argument edge transfer, and structured `scf.if`/`scf.for`
//! transfer.

use std::collections::{HashMap, HashSet, VecDeque};

use kirin_ir::{Block, Dialect, GetInfo, Pipeline, SSAValue, StageInfo, Statement};

use crate::{Edge, Flow, LiveSet, Liveness, LivenessError, LivenessOp};

/// Compute liveness for a single function, given its stage arena and the
/// statement that is the function body (the op exposing the body region).
///
/// This is the engine entry point used by callers that already hold a typed
/// [`StageInfo`] and the body statement. For pipeline-by-name resolution, see
/// [`analyze_liveness_by_name`].
///
/// # Block-argument convention
///
/// A successor's live block parameters appear in its [`Liveness::block_in`];
/// the predecessor's branch maps them back to the concrete edge args (so a
/// value passed on an edge is live before the branch exactly when the matching
/// target parameter is live at the target).
pub fn analyze_function<L>(stage: &StageInfo<L>, body: Statement) -> Result<Liveness, LivenessError>
where
    L: Dialect + LivenessOp,
{
    let mut solver = Solver::new(stage);
    solver.run(body)?;
    Ok(solver.result)
}

/// Resolve a function by stage/function name in a single-language pipeline and
/// compute its liveness.
pub fn analyze_liveness_by_name<L>(
    pipeline: &Pipeline<StageInfo<L>>,
    stage_name: &str,
    function_name: &str,
) -> Result<Liveness, LivenessError>
where
    L: Dialect + LivenessOp,
{
    let stage = pipeline
        .stage_by_name(stage_name)
        .ok_or_else(|| LivenessError::MissingStage(stage_name.to_string()))?;
    let info = pipeline
        .stage(stage)
        .ok_or_else(|| LivenessError::MissingStage(stage_name.to_string()))?;
    let staged = pipeline
        .resolve_staged_function(function_name, stage)
        .ok_or_else(|| LivenessError::MissingFunction(function_name.to_string()))?;
    let staged_info = staged
        .get_info(info)
        .ok_or_else(|| LivenessError::MissingFunction(function_name.to_string()))?;
    let specialized = staged_info
        .unique_live_specialization()
        .map_err(|err| LivenessError::Specialization(err.to_string()))?;
    let body = *specialized
        .get_info(info)
        .ok_or_else(|| LivenessError::MissingFunction(function_name.to_string()))?
        .body();
    analyze_function(info, body)
}

struct Solver<'ir, L: Dialect + LivenessOp> {
    stage: &'ir StageInfo<L>,
    result: Liveness,
}

impl<'ir, L: Dialect + LivenessOp> Solver<'ir, L> {
    fn new(stage: &'ir StageInfo<L>) -> Self {
        Self {
            stage,
            result: Liveness::default(),
        }
    }

    // ---- IR fact helpers (return owned data to keep borrows local) ----------

    fn uses_of(&self, stmt: Statement) -> Vec<SSAValue> {
        stmt.arguments(self.stage).copied().collect()
    }

    fn defs_of(&self, stmt: Statement) -> Vec<SSAValue> {
        stmt.results(self.stage)
            .map(|r| SSAValue::from(*r))
            .collect()
    }

    fn block_params(&self, block: Block) -> Vec<SSAValue> {
        match block.get_info(self.stage) {
            Some(info) => info.arguments.iter().copied().map(SSAValue::from).collect(),
            None => Vec::new(),
        }
    }

    fn is_pure(&self, stmt: Statement) -> bool {
        stmt.definition(self.stage).is_pure()
    }

    fn flow(&self, stmt: Statement) -> Flow {
        stmt.definition(self.stage).flow()
    }

    fn set_before(&mut self, stmt: Statement, set: LiveSet) {
        self.result.stmt_before.insert(stmt, set);
    }

    fn set_after(&mut self, stmt: Statement, set: LiveSet) {
        self.result.stmt_after.insert(stmt, set);
    }

    // ---- CFG worklist -------------------------------------------------------

    fn run(&mut self, body: Statement) -> Result<(), LivenessError> {
        let stage = self.stage;
        let region = match body.regions(stage).next() {
            Some(region) => *region,
            None => return Err(LivenessError::NoBody(body)),
        };
        let blocks: Vec<Block> = region.blocks(stage).collect();
        if blocks.is_empty() {
            return Ok(());
        }

        // Classify each top-level block's terminator into successor edges and
        // build the predecessor map.
        let mut succ: HashMap<Block, Vec<Edge>> = HashMap::new();
        let mut preds: HashMap<Block, Vec<Block>> = HashMap::new();
        for &block in &blocks {
            let edges = match block.terminator(stage) {
                Some(term) => match self.flow(term) {
                    Flow::Branch(edges) => edges,
                    Flow::Return => Vec::new(),
                    _ => return Err(LivenessError::UnsupportedTerminator(term)),
                },
                None => Vec::new(),
            };
            for edge in &edges {
                preds.entry(edge.target).or_default().push(block);
            }
            succ.insert(block, edges);
        }

        // Backward worklist to fixpoint. The lattice is a finite powerset under
        // union, so iteration terminates without widening.
        for &block in &blocks {
            self.result.block_in.insert(block, LiveSet::new());
        }
        let mut worklist: VecDeque<Block> = blocks.iter().rev().copied().collect();
        let mut queued: HashSet<Block> = blocks.iter().copied().collect();
        while let Some(block) = worklist.pop_front() {
            queued.remove(&block);
            let out = self.edge_union(&succ[&block])?;
            let new_in = self.walk_cfg_block_backward(block, out.clone())?;
            self.result.block_out.insert(block, out);
            let changed = self.result.block_in.get(&block) != Some(&new_in);
            self.result.block_in.insert(block, new_in);
            if changed && let Some(predecessors) = preds.get(&block) {
                for &pred in predecessors {
                    if queued.insert(pred) {
                        worklist.push_back(pred);
                    }
                }
            }
        }
        Ok(())
    }

    /// `block_out[b]` = union over successor edges of the edge transfer, which
    /// maps each live target block parameter back to the value passed on that
    /// edge and passes free (non-parameter) values through unchanged.
    fn edge_union(&self, edges: &[Edge]) -> Result<LiveSet, LivenessError> {
        let mut out = LiveSet::new();
        for edge in edges {
            let target_in = self
                .result
                .block_in
                .get(&edge.target)
                .cloned()
                .unwrap_or_default();
            let params = self.block_params(edge.target);
            if edge.args.len() != params.len() {
                return Err(LivenessError::MalformedEdge {
                    edge_args: edge.args.len(),
                    block_params: params.len(),
                });
            }
            for (index, param) in params.iter().enumerate() {
                if target_in.contains(param) {
                    out.insert(edge.args[index]);
                }
            }
            let param_set: HashSet<SSAValue> = params.iter().copied().collect();
            for value in target_in.iter() {
                if !param_set.contains(value) {
                    out.insert(*value);
                }
            }
        }
        Ok(out)
    }

    fn walk_cfg_block_backward(
        &mut self,
        block: Block,
        block_out: LiveSet,
    ) -> Result<LiveSet, LivenessError> {
        let stage = self.stage;
        let mut current = block_out;

        if let Some(term) = block.terminator(stage) {
            match self.flow(term) {
                Flow::Branch(edges) => {
                    // Edge args are accounted for in `block_out` already; the
                    // terminator only adds its direct (non-edge) uses, e.g. a
                    // conditional branch's condition.
                    let edge_args: HashSet<SSAValue> =
                        edges.iter().flat_map(|e| e.args.iter().copied()).collect();
                    self.set_after(term, current.clone());
                    for value in self.uses_of(term) {
                        if !edge_args.contains(&value) {
                            current.insert(value);
                        }
                    }
                    self.set_before(term, current.clone());
                }
                Flow::Return => {
                    self.set_after(term, current.clone());
                    for value in self.uses_of(term) {
                        current.insert(value);
                    }
                    self.set_before(term, current.clone());
                }
                _ => return Err(LivenessError::UnsupportedTerminator(term)),
            }
        }

        let statements: Vec<Statement> = block.statements(stage).collect();
        for stmt in statements.into_iter().rev() {
            self.set_after(stmt, current.clone());
            current = self.transfer_stmt(stmt, &current)?;
            self.set_before(stmt, current.clone());
        }
        Ok(current)
    }

    // ---- statement transfer -------------------------------------------------

    fn transfer_stmt(
        &mut self,
        stmt: Statement,
        live_after: &LiveSet,
    ) -> Result<LiveSet, LivenessError> {
        match self.flow(stmt) {
            Flow::Plain => {
                // A statement carrying nested control flow that we do not model
                // is an explicit error rather than a silent approximation.
                if stmt.blocks(self.stage).next().is_some()
                    || stmt.regions(self.stage).next().is_some()
                {
                    return Err(LivenessError::UnsupportedStructuredControlFlow(stmt));
                }
                Ok(self.plain_transfer(stmt, live_after))
            }
            Flow::If {
                condition,
                then_block,
                else_block,
                results,
            } => self.transfer_if(
                stmt, live_after, condition, then_block, else_block, &results,
            ),
            Flow::For {
                start,
                end,
                step,
                init_args,
                body,
                results,
            } => self.transfer_for(
                stmt, live_after, start, end, step, &init_args, body, &results,
            ),
            // Terminator-only shapes must not appear as a body statement.
            Flow::Branch(_) | Flow::Return | Flow::Yield { .. } => {
                Err(LivenessError::UnsupportedTerminator(stmt))
            }
        }
    }

    /// `live_before = uses ∪ (live_after − defs)`, with the refinement that a
    /// pure op whose results are all dead does not make its operands live.
    fn plain_transfer(&self, stmt: Statement, live_after: &LiveSet) -> LiveSet {
        let defs = self.defs_of(stmt);
        if self.is_pure(stmt) && defs.iter().all(|d| !live_after.contains(d)) {
            return live_after.clone();
        }
        let mut out = live_after.clone();
        for def in &defs {
            out.remove(def);
        }
        for use_value in self.uses_of(stmt) {
            out.insert(use_value);
        }
        out
    }

    // ---- structured control flow -------------------------------------------

    /// Yield values of `body`'s terminator. `scf_stmt` provides context for
    /// errors when the body is malformed.
    fn yield_values(
        &self,
        body: Block,
        scf_stmt: Statement,
    ) -> Result<Vec<SSAValue>, LivenessError> {
        let term = body
            .terminator(self.stage)
            .ok_or(LivenessError::UnsupportedStructuredControlFlow(scf_stmt))?;
        match self.flow(term) {
            Flow::Yield { values } => Ok(values),
            _ => Err(LivenessError::UnsupportedTerminator(term)),
        }
    }

    /// Backward walk of a single `scf` body block, given the set live just
    /// before its `yield`. Records the body's statements (including the yield)
    /// and returns the body's live-in.
    fn walk_scf_body_backward(
        &mut self,
        body: Block,
        yield_live: LiveSet,
    ) -> Result<LiveSet, LivenessError> {
        let stage = self.stage;
        if let Some(term) = body.terminator(stage) {
            self.set_after(term, yield_live.clone());
            self.set_before(term, yield_live.clone());
        }
        self.result.block_out.insert(body, yield_live.clone());

        let mut current = yield_live;
        let statements: Vec<Statement> = body.statements(stage).collect();
        for stmt in statements.into_iter().rev() {
            self.set_after(stmt, current.clone());
            current = self.transfer_stmt(stmt, &current)?;
            self.set_before(stmt, current.clone());
        }
        self.result.block_in.insert(body, current.clone());
        Ok(current)
    }

    #[allow(clippy::too_many_arguments)]
    fn transfer_if(
        &mut self,
        stmt: Statement,
        live_after: &LiveSet,
        condition: SSAValue,
        then_block: Block,
        else_block: Block,
        results: &[SSAValue],
    ) -> Result<LiveSet, LivenessError> {
        let then_in = self.analyze_if_arm(stmt, then_block, results, live_after)?;
        let else_in = self.analyze_if_arm(stmt, else_block, results, live_after)?;

        let mut out = LiveSet::new();
        out.insert(condition);
        out.union_with(&then_in);
        out.union_with(&else_in);
        // Non-result values live after the `if` flow around it unchanged.
        let results_set: HashSet<SSAValue> = results.iter().copied().collect();
        for value in live_after.iter() {
            if !results_set.contains(value) {
                out.insert(*value);
            }
        }
        Ok(out)
    }

    /// A yielded value is live at the end of an arm exactly when the matching
    /// result of the `if` is live after it.
    fn analyze_if_arm(
        &mut self,
        stmt: Statement,
        arm: Block,
        results: &[SSAValue],
        live_after: &LiveSet,
    ) -> Result<LiveSet, LivenessError> {
        let yields = self.yield_values(arm, stmt)?;
        let mut yield_live = LiveSet::new();
        for (index, yielded) in yields.iter().enumerate() {
            if results
                .get(index)
                .is_some_and(|result| live_after.contains(result))
            {
                yield_live.insert(*yielded);
            }
        }
        self.walk_scf_body_backward(arm, yield_live)
    }

    #[allow(clippy::too_many_arguments)]
    fn transfer_for(
        &mut self,
        stmt: Statement,
        live_after: &LiveSet,
        start: SSAValue,
        end: SSAValue,
        step: SSAValue,
        init_args: &[SSAValue],
        body: Block,
        results: &[SSAValue],
    ) -> Result<LiveSet, LivenessError> {
        let body_params = self.block_params(body);
        let yields = self.yield_values(body, stmt)?;

        // Loop-carried fixpoint: a yielded value `y_k` is needed when the
        // matching carried parameter `c_k` is live at body entry (next
        // iteration) OR the matching result `r_k` is live after the loop
        // (exit). `body_in` grows monotonically to a fixpoint.
        let mut body_in = LiveSet::new();
        loop {
            let yield_live =
                self.for_yield_live(&yields, &body_params, results, live_after, &body_in);
            let new_body_in = self.walk_scf_body_backward(body, yield_live)?;
            if new_body_in == body_in {
                break;
            }
            body_in = new_body_in;
        }

        let mut out = LiveSet::new();
        out.insert(start);
        out.insert(end);
        out.insert(step);
        // Free values used in the body (everything live at entry that is not a
        // body parameter) are live before the loop.
        let param_set: HashSet<SSAValue> = body_params.iter().copied().collect();
        for value in body_in.iter() {
            if !param_set.contains(value) {
                out.insert(*value);
            }
        }
        // An init value is needed when its carried slot is used in the body, or
        // when its result is live after a possibly-zero-iteration loop.
        for (index, init) in init_args.iter().enumerate() {
            let carried_live = body_params
                .get(index + 1)
                .is_some_and(|carried| body_in.contains(carried));
            let result_live = results
                .get(index)
                .is_some_and(|result| live_after.contains(result));
            if carried_live || result_live {
                out.insert(*init);
            }
        }
        // Non-result values live after the loop flow around it unchanged.
        let results_set: HashSet<SSAValue> = results.iter().copied().collect();
        for value in live_after.iter() {
            if !results_set.contains(value) {
                out.insert(*value);
            }
        }
        Ok(out)
    }

    fn for_yield_live(
        &self,
        yields: &[SSAValue],
        body_params: &[SSAValue],
        results: &[SSAValue],
        live_after: &LiveSet,
        body_in: &LiveSet,
    ) -> LiveSet {
        let mut yield_live = LiveSet::new();
        for (index, yielded) in yields.iter().enumerate() {
            let carried_live = body_params
                .get(index + 1)
                .is_some_and(|carried| body_in.contains(carried));
            let result_live = results
                .get(index)
                .is_some_and(|result| live_after.contains(result));
            if carried_live || result_live {
                yield_live.insert(*yielded);
            }
        }
        yield_live
    }
}
