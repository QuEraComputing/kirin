use kirin_ir::{CompileStage, Product, SSAValue, Statement};

use crate::{
    EnvIndex, FrameDriver, FrameEffect, InterpreterError, SparseForwardEffect, SparseForwardInterp,
};

use super::{CallFrame, Completion, FrameBuild};

/// Representation-specific walker for a [`DiGraph`](kirin_ir::DiGraph) body: bind
/// entry arguments to the graph's boundary ports, run the node statements in
/// topological (dependency) order, and complete
/// [`Finished`](Completion::Finished) with the graph's declared yields.
///
/// Traversal mechanics only. A `DiGraphFrame` does not own an activation and
/// does not know whether it is a callable graph-function body
/// ([`CallFrame`] → `DiGraphFrame`, where the yields become the call's
/// returned values) or a graph nested inside another body (dialect frame →
/// `DiGraphFrame`, where the yields return to the pushing operation): the
/// parent interprets the completion.
///
/// The concrete execution policy requires a DAG: directed cycles are
/// rejected when the walk plan is built
/// ([`GraphHasCycle`](InterpreterError::GraphHasCycle)). This is a property
/// of this walker, not of the IR — a `DiGraph` may represent cycles.
///
/// Pure construction — the walk plan is fetched and the ports are bound on
/// the first `step`, so a dialect frame can build one without engine access
/// (the same lazy pattern as [`BlockFrame`](super::BlockFrame)). CFG control
/// flow (`Jump`/`Branch`) and `Yield`/`Return` are errors inside a graph: a
/// digraph's outputs are its declared yields, not a statement effect.
pub struct DiGraphFrame<V, E> {
    stage: CompileStage,
    index: EnvIndex,
    graph: kirin_ir::DiGraph,
    /// Entry arguments not yet bound (bound on the first `step`).
    pending: Option<Product<V>>,
    /// Remaining schedule, in topological order; `None` until the first step.
    schedule: Option<std::collections::VecDeque<Statement>>,
    yields: Vec<SSAValue>,
    /// Result slots awaiting a pushed child frame's completion values.
    resume_slots: Option<Product<SSAValue>>,
    _marker: std::marker::PhantomData<fn() -> (V, E)>,
}

impl<V, E> DiGraphFrame<V, E>
where
    V: Clone,
    E: From<InterpreterError>,
{
    /// Walk `graph`, binding `args` to its boundary ports on the first step.
    pub fn new(
        stage: CompileStage,
        index: EnvIndex,
        graph: kirin_ir::DiGraph,
        args: Product<V>,
    ) -> Self {
        Self {
            stage,
            index,
            graph,
            pending: Some(args),
            schedule: None,
            yields: Vec::new(),
            resume_slots: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// Execute the next scheduled node and translate its
    /// [`SparseForwardEffect`] into a [`FrameEffect`] over the total frame
    /// type `F`.
    pub fn step_into<I, F>(mut self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = F>,
        F: FrameBuild<V, E>,
    {
        // First step: fetch the walk plan and bind the boundary ports.
        if let Some(args) = self.pending.take() {
            let plan = interp.digraph_walk_plan(self.stage, self.graph)?;
            if plan.ports.len() != args.len() {
                return Err(E::from(InterpreterError::ProductArityMismatch {
                    expected: plan.ports.len(),
                    actual: args.len(),
                }));
            }
            for (port, value) in plan.ports.iter().copied().zip(args) {
                interp.env_write(self.index, SSAValue::from(port), value)?;
            }
            self.schedule = Some(plan.schedule.into());
            self.yields = plan.yields;
            return Ok(FrameEffect::Continue(F::from_digraph(self)));
        }

        let Some(statement) = self.schedule.as_mut().and_then(|s| s.pop_front()) else {
            return self.finish::<I, F>(interp);
        };

        match interp.run_statement(self.stage, statement, self.index)? {
            SparseForwardEffect::Next => Ok(FrameEffect::Continue(F::from_digraph(self))),
            SparseForwardEffect::Push { frame, results } => {
                self.resume_slots = Some(results);
                Ok(FrameEffect::Push {
                    parent: F::from_digraph(self),
                    child: frame,
                })
            }
            SparseForwardEffect::Call(call) => {
                let pending = CallFrame::pending(self.stage, self.index, call);
                Ok(FrameEffect::Push {
                    parent: F::from_digraph(self),
                    child: F::from_call(pending),
                })
            }
            SparseForwardEffect::Jump(_) | SparseForwardEffect::Branch(_) => {
                Err(E::from(InterpreterError::CFGControlFlowInStructuredBody))
            }
            SparseForwardEffect::Yield(_) => Err(E::from(InterpreterError::Custom(
                "yield inside a digraph body (a digraph's outputs are its declared yields)",
            ))),
            SparseForwardEffect::Return(_) => Err(E::from(InterpreterError::Custom(
                "return inside a digraph body",
            ))),
        }
    }

    /// Schedule exhausted: read the declared yields from the activation and
    /// complete `Finished` — the graph's natural completion. The parent
    /// decides what the values mean (call returns or push results).
    fn finish<I, F>(self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E>,
        F: FrameBuild<V, E>,
    {
        let values: Product<V> = self
            .yields
            .iter()
            .map(|&value| interp.env_read(self.index, value))
            .collect::<Result<_, _>>()?;
        Ok(FrameEffect::Complete(Completion::Finished(values)))
    }

    /// A child finished without a payload (e.g. a returned call whose results
    /// are already written): resume the schedule.
    pub fn resume_done_into<F>(self) -> FrameEffect<F, Completion<V>>
    where
        F: FrameBuild<V, E>,
    {
        FrameEffect::Continue(F::from_digraph(self))
    }

    /// A child bubbled a completion: a pushed frame's values land in the
    /// push's result slots. A `Returned` cannot bubble out of a graph node —
    /// a digraph has no function-return convention.
    pub fn resume_into<I, F>(
        mut self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E>,
        F: FrameBuild<V, E>,
    {
        match completion {
            Completion::Finished(values) | Completion::Yielded(values) => {
                let slots = self.resume_slots.take().ok_or_else(|| {
                    E::from(InterpreterError::Custom(
                        "digraph resume without result slots",
                    ))
                })?;
                interp.write_results(self.index, &slots, values)?;
                Ok(FrameEffect::Continue(F::from_digraph(self)))
            }
            Completion::Returned(_) => Err(E::from(InterpreterError::Custom(
                "return bubbled into a digraph body",
            ))),
        }
    }
}
