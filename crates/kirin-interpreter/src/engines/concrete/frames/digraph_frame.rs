use kirin_ir::{CompileStage, Product, SSAValue, Statement};

use crate::{
    DiGraphQueries, Env, EnvIndex, Frame, FrameEffect, InterpreterError, SparseForwardEffect,
    SparseForwardInterp, StatementDispatch,
};

use super::{CallRequest, Completion};

/// Representation-specific walker for a [`DiGraph`](kirin_ir::DiGraph) body: bind
/// entry arguments to the graph's boundary ports, run the node statements in
/// topological (dependency) order, and complete
/// [`Finished`](Completion::Finished) with the graph's declared yields.
///
/// Traversal mechanics only. A `DiGraphFrame` does not own an activation and
/// does not know whether it is a callable graph-function body
/// ([`CallFrame`](crate::CallFrame) → `DiGraphFrame`, where the yields become the call's
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

    /// Schedule exhausted: read the declared yields from the activation and
    /// complete `Finished` — the graph's natural completion. The parent
    /// decides what the values mean (call returns or push results).
    ///
    /// Reading the yields is all this step needs, so it asks for [`Env`] alone —
    /// not [`DiGraphQueries`], whose schedule was already consumed.
    fn finish<I, R>(self, interp: &mut I) -> Result<FrameEffect<Self, Completion<V>, R>, E>
    where
        I: Env<Value = V, Error = E>,
    {
        let values: Product<V> = self
            .yields
            .iter()
            .map(|&value| interp.env_read(self.index, value))
            .collect::<Result<_, _>>()?;
        Ok(FrameEffect::Complete(Completion::Finished(values)))
    }
}

impl<I, R, V, E> Frame<I, Self, R> for DiGraphFrame<V, E>
where
    I: DiGraphQueries<Value = V, Error = E> + StatementDispatch + SparseForwardInterp<Frame = R>,
    R: From<CallRequest<V>>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    /// Execute the next scheduled node and translate its
    /// [`SparseForwardEffect`] into a [`FrameEffect`] over the total frame
    /// stack-item type selected by the engine configuration.
    fn step_into(mut self, interp: &mut I) -> Result<FrameEffect<Self, Completion<V>, R>, E> {
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
            return Ok(FrameEffect::Continue(self));
        }

        let Some(statement) = self.schedule.as_mut().and_then(|s| s.pop_front()) else {
            return self.finish::<I, R>(interp);
        };

        match interp.run_statement(self.stage, statement, self.index)? {
            SparseForwardEffect::Next => Ok(FrameEffect::Continue(self)),
            SparseForwardEffect::Push { frame, results } => {
                self.resume_slots = Some(results);
                Ok(FrameEffect::Push {
                    parent: self,
                    child: frame,
                })
            }
            SparseForwardEffect::Call(call) => {
                let pending = CallRequest::pending(self.stage, self.index, call);
                Ok(FrameEffect::Push {
                    parent: self,
                    child: pending.into(),
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

    /// A child finished without a payload (e.g. a returned call whose results
    /// are already written): resume the schedule.
    fn resume_done_into(self, _interp: &mut I) -> Result<FrameEffect<Self, Completion<V>, R>, E> {
        Ok(FrameEffect::Continue(self))
    }

    /// A child bubbled a completion: a pushed frame's values land in the
    /// push's result slots. A `Returned` cannot bubble out of a graph node —
    /// a digraph has no function-return convention.
    fn resume_into(
        mut self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Completion<V>, R>, E> {
        match completion {
            Completion::Finished(values) | Completion::Yielded(values) => {
                let slots = self.resume_slots.take().ok_or_else(|| {
                    E::from(InterpreterError::Custom(
                        "digraph resume without result slots",
                    ))
                })?;
                interp.bind_values(self.index, slots.as_slice(), values)?;
                Ok(FrameEffect::Continue(self))
            }
            Completion::Returned(_) => Err(E::from(InterpreterError::Custom(
                "return bubbled into a digraph body",
            ))),
        }
    }
}
