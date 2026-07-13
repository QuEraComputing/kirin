//! The **concrete** implementation of the shared [`frame`](crate::core::frame)
//! protocol.
//!
//! These are the default total frames for [`ConcreteInterpreter`](crate::ConcreteInterpreter):
//! [`BodyFrame`] (walks a function-body CFG or a single body block) and
//! [`CallFrame`] (call/return). They implement the shared [`Frame`] trait by
//! consuming the dialect [`SparseForwardEffect`] and driving a single deterministic
//! path. Structured-control dialects do not get a framework "scope": they push
//! a frame **they own** through [`SparseForwardEffect::Push`] (that frame may build a
//! [`BodyFrame`] to walk a chosen body — a reusable building block, not
//! framework-owned structured semantics). A language that combines such a
//! dialect defines its own total frame enum embedding [`BodyFrame`]/[`CallFrame`]
//! via [`FrameBuild`] plus its dialect frames. The forward abstract analogue
//! lives in [`sparse_forward::frames`](crate::engines::sparse_forward::frames).

use kirin_ir::{Block, Cfg, CompileStage, Product, SSAValue, Statement};

use crate::{
    Body, CallEffect, Callee, EnvIndex, Frame, FrameDriver, FrameEffect, InterpreterError,
    SparseForwardEffect, SparseForwardInterp,
};

/// Completion payloads produced by the standard concrete frames.
///
/// `Returned` bubbles a function return across frames to the enclosing
/// [`CallFrame`]; `Finished` carries the values a pushed body frame yielded back
/// to whoever pushed it (written into that push's result slots).
pub enum Completion<V> {
    /// A function returned these values; bubbles to the enclosing
    /// [`CallFrame`], or finishes the run at the root.
    Returned(Product<V>),
    /// A pushed body frame yielded these values to its pusher.
    Finished(Product<V>),
}

/// Construction trait letting any total frame enum embed the standard concrete
/// frames.
///
/// The default [`StandardFrame`] implements it trivially; a language that adds
/// structured-control dialects implements it on its own enum to reuse
/// [`BodyFrame`]/[`CallFrame`] traversal while adding its own dialect frames.
pub trait FrameBuild<V, E>: Sized {
    fn from_body(frame: BodyFrame<V, E>) -> Self;
    fn from_call(frame: CallFrame<V>) -> Self;
    fn from_digraph(frame: DiGraphFrame<V, E>) -> Self;
}

/// Traversal of one body: a function-body CFG (multi-block, with jumps)
/// or a single body block (scf-style, terminated by a yield).
/// How a [`BodyFrame`] treats its current block's control flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockMode {
    /// A block belonging to a CFG: `Jump`/`Branch` move between blocks.
    CfgBlock,
    /// A structured (scf-style) or linear-function body: a single block whose
    /// exit is `Yield` (structured) or `Return` (linear function);
    /// `Jump`/`Branch` are an error.
    StructuredBody,
}

pub struct BodyFrame<V, E> {
    stage: CompileStage,
    index: EnvIndex,
    owns_env: bool,
    function_boundary: bool,
    mode: BlockMode,
    block: Block,
    cursor: Option<Statement>,
    /// Entry arguments not yet bound. A body frame built by a dialect frame is
    /// constructed without engine access — it binds on its first `step`, so
    /// building it requires no [`FrameDriver`] (a dialect frame builds these as
    /// plain values, no engine capability or trait-resolution cycle).
    pending: Option<Product<V>>,
    /// Result slots awaiting a pushed body frame's `Finished` completion.
    resume_slots: Option<Product<SSAValue>>,
    _marker: std::marker::PhantomData<fn() -> (V, E)>,
}

impl<V, E> BodyFrame<V, E>
where
    V: Clone,
    E: From<InterpreterError>,
{
    /// Walk a function body: start at the entry block of `cfg`, binding
    /// `args` to its parameters. Owns the activation and is the return boundary.
    pub fn function<I>(
        interp: &mut I,
        stage: CompileStage,
        index: EnvIndex,
        cfg: Cfg,
        args: Product<V>,
    ) -> Result<Self, E>
    where
        I: FrameDriver<Value = V, Error = E>,
    {
        let entry = interp
            .cfg_entry(stage, cfg)?
            .ok_or_else(|| E::from(InterpreterError::EmptyCfg))?;
        Self::start(
            interp,
            stage,
            index,
            entry,
            args,
            true,
            true,
            BlockMode::CfgBlock,
        )
    }

    /// Walk a linear (single-`Block`) function body: bind `args` to the
    /// block's parameters. Owns the activation and is the return boundary;
    /// `Jump`/`Branch`/`Yield` are errors — the exit convention is `Return`.
    pub fn linear_function<I>(
        interp: &mut I,
        stage: CompileStage,
        index: EnvIndex,
        block: Block,
        args: Product<V>,
    ) -> Result<Self, E>
    where
        I: FrameDriver<Value = V, Error = E>,
    {
        Self::start(
            interp,
            stage,
            index,
            block,
            args,
            true,
            true,
            BlockMode::StructuredBody,
        )
    }

    /// A single body block (scf-style), to bind `args` to its parameters on the
    /// first step. Borrows the caller's activation and is not a return boundary.
    /// Pure construction — needs no engine access.
    pub fn block(stage: CompileStage, index: EnvIndex, block: Block, args: Product<V>) -> Self {
        Self {
            stage,
            index,
            owns_env: false,
            function_boundary: false,
            mode: BlockMode::StructuredBody,
            block,
            cursor: None,
            pending: Some(args),
            resume_slots: None,
            _marker: std::marker::PhantomData,
        }
    }

    fn start<I>(
        interp: &mut I,
        stage: CompileStage,
        index: EnvIndex,
        block: Block,
        args: Product<V>,
        owns_env: bool,
        function_boundary: bool,
        mode: BlockMode,
    ) -> Result<Self, E>
    where
        I: FrameDriver<Value = V, Error = E>,
    {
        interp.bind_block_args(stage, index, block, &args)?;
        let cursor = interp.first_statement(stage, block)?;
        Ok(Self {
            stage,
            index,
            owns_env,
            function_boundary,
            mode,
            block,
            cursor,
            pending: None,
            resume_slots: None,
            _marker: std::marker::PhantomData,
        })
    }

    /// Execute the next statement and translate its [`SparseForwardEffect`] into a
    /// [`FrameEffect`] over the total frame type `F`.
    pub fn step_into<I, F>(mut self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = F>,
        F: FrameBuild<V, E>,
    {
        // Bind entry arguments lazily on the first step (a dialect-built body
        // frame carries them unbound).
        if let Some(args) = self.pending.take() {
            interp.bind_block_args(self.stage, self.index, self.block, &args)?;
            self.cursor = interp.first_statement(self.stage, self.block)?;
            return Ok(FrameEffect::Continue(F::from_body(self)));
        }
        let Some(statement) = self.cursor else {
            return Err(E::from(if self.function_boundary {
                InterpreterError::FunctionBodyFellThrough
            } else {
                InterpreterError::BlockFellThrough(self.block)
            }));
        };
        self.cursor = interp.next_statement(self.stage, self.block, statement)?;

        match interp.run_statement(self.stage, statement, self.index)? {
            SparseForwardEffect::Next => Ok(FrameEffect::Continue(F::from_body(self))),
            SparseForwardEffect::Jump(edge) => {
                if self.mode == BlockMode::StructuredBody {
                    return Err(E::from(InterpreterError::CfgControlFlowInStructuredBody));
                }
                interp.bind_block_args(self.stage, self.index, edge.target, &edge.args)?;
                self.cursor = interp.first_statement(self.stage, edge.target)?;
                self.block = edge.target;
                Ok(FrameEffect::Continue(F::from_body(self)))
            }
            SparseForwardEffect::Branch(_) => {
                if self.mode == BlockMode::StructuredBody {
                    return Err(E::from(InterpreterError::CfgControlFlowInStructuredBody));
                }
                Err(E::from(InterpreterError::IndeterminateBranch))
            }
            SparseForwardEffect::Push { frame, results } => {
                self.resume_slots = Some(results);
                Ok(FrameEffect::Push {
                    parent: F::from_body(self),
                    child: frame,
                })
            }
            SparseForwardEffect::Call(call) => {
                let pending = CallFrame::pending(self.stage, self.index, call);
                Ok(FrameEffect::Push {
                    parent: F::from_body(self),
                    child: F::from_call(pending),
                })
            }
            SparseForwardEffect::Yield(values) => {
                if self.function_boundary {
                    return Err(E::from(InterpreterError::Custom(
                        "yield reached a function boundary",
                    )));
                }
                Ok(FrameEffect::Complete(Completion::Finished(values)))
            }
            SparseForwardEffect::Return(values) => self.finish_return::<I, F>(interp, values),
        }
    }

    /// A child finished without a payload (its results are already in the
    /// shared index, e.g. a returned call): resume at the advanced cursor.
    pub fn resume_done_into<F>(self) -> FrameEffect<F, Completion<V>>
    where
        F: FrameBuild<V, E>,
    {
        FrameEffect::Continue(F::from_body(self))
    }

    /// A child bubbled a completion: a pushed body frame `Finished` (write its
    /// values into the pending slots and continue) or a `Returned` (a return
    /// happened in the child — keep bubbling).
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
            Completion::Finished(values) => {
                let slots = self.resume_slots.take().ok_or_else(|| {
                    E::from(InterpreterError::Custom("body resume without result slots"))
                })?;
                interp.write_results(self.index, &slots, values)?;
                Ok(FrameEffect::Continue(F::from_body(self)))
            }
            Completion::Returned(values) => self.finish_return::<I, F>(interp, values),
        }
    }

    /// Produce a `Returned` completion, freeing the activation record when this
    /// frame is the owning function boundary.
    fn finish_return<I, F>(
        self,
        interp: &mut I,
        values: Product<V>,
    ) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E>,
        F: FrameBuild<V, E>,
    {
        if self.function_boundary && self.owns_env {
            interp.free_env(self.index)?;
        }
        Ok(FrameEffect::Complete(Completion::Returned(values)))
    }
}

/// Call/return bookkeeping: dispatch a function invocation, then await its
/// return and land the results in the caller's activation.
pub enum CallFrame<V> {
    /// Not yet dispatched: resolve the callee, enter its body.
    Pending {
        resolve_stage: CompileStage,
        callee: Callee,
        args: Product<V>,
        caller_env: EnvIndex,
        results: Product<SSAValue>,
    },
    /// Dispatched: awaiting the callee's `Returned` completion.
    Awaiting {
        caller_env: EnvIndex,
        results: Product<SSAValue>,
    },
}

impl<V> CallFrame<V>
where
    V: Clone,
{
    /// Build a pending call frame from a [`CallEffect`].
    pub fn pending(scope_stage: CompileStage, caller_env: EnvIndex, call: CallEffect<V>) -> Self {
        CallFrame::Pending {
            resolve_stage: call.stage.unwrap_or(scope_stage),
            callee: call.callee,
            args: call.args,
            caller_env,
            results: call.results,
        }
    }

    pub fn step_into<I, F>(self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, I::Error>
    where
        I: FrameDriver<Value = V>,
        I::Error: From<InterpreterError>,
        F: FrameBuild<V, I::Error>,
    {
        match self {
            CallFrame::Pending {
                resolve_stage,
                callee,
                args,
                caller_env,
                results,
            } => {
                let target = interp.resolve_call(resolve_stage, &callee)?;
                let index = interp.alloc_env();
                let body = interp.enter_function(target.stage, target.body, args, index)?;
                let child = match body.body {
                    Body::Cfg(cfg) => F::from_body(BodyFrame::function(
                        interp,
                        target.stage,
                        index,
                        cfg,
                        body.args,
                    )?),
                    Body::Block(block) => F::from_body(BodyFrame::linear_function(
                        interp,
                        target.stage,
                        index,
                        block,
                        body.args,
                    )?),
                    Body::DiGraph(graph) => F::from_digraph(DiGraphFrame::function(
                        target.stage,
                        index,
                        graph,
                        body.args,
                    )),
                    other @ Body::UnGraph(_) => {
                        return Err(I::Error::from(InterpreterError::NoDefaultWalker(other)));
                    }
                };
                Ok(FrameEffect::Push {
                    parent: F::from_call(CallFrame::Awaiting {
                        caller_env,
                        results,
                    }),
                    child,
                })
            }
            CallFrame::Awaiting { .. } => Err(I::Error::from(InterpreterError::Custom(
                "call frame stepped while awaiting a return",
            ))),
        }
    }

    pub fn resume_done_into<F>(self) -> Result<FrameEffect<F, Completion<V>>, InterpreterError> {
        Err(InterpreterError::Custom(
            "call frame resumed without a return",
        ))
    }

    pub fn resume_into<I, F>(
        self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, Completion<V>>, I::Error>
    where
        I: FrameDriver<Value = V>,
        I::Error: From<InterpreterError>,
        F: FrameBuild<V, I::Error>,
    {
        match (self, completion) {
            (
                CallFrame::Awaiting {
                    caller_env,
                    results,
                },
                Completion::Returned(values),
            ) => {
                interp.write_results(caller_env, &results, values)?;
                Ok(FrameEffect::Done)
            }
            (CallFrame::Awaiting { .. }, Completion::Finished(_)) => Err(I::Error::from(
                InterpreterError::Custom("call frame resumed with a body completion"),
            )),
            (CallFrame::Pending { .. }, _) => Err(I::Error::from(InterpreterError::Custom(
                "call frame resumed before dispatch",
            ))),
        }
    }
}

/// The default total concrete frame enum: standard concrete traversal (no
/// structured-control dialect frames).
/// Traversal of one digraph body: bind entry arguments to the graph's
/// boundary ports, run the node statements in topological order, and
/// complete with the graph's yielded values.
///
/// Pure construction — the walk plan is fetched and the ports are bound on
/// the first `step`, so a dialect frame can build one without engine access
/// (the same lazy pattern as [`BodyFrame::block`]). CFG control flow
/// (`Jump`/`Branch`) and `Yield`/`Return` are errors inside a graph: a
/// digraph's outputs are its declared yields, not a statement effect.
pub struct DiGraphFrame<V, E> {
    stage: CompileStage,
    index: EnvIndex,
    owns_env: bool,
    function_boundary: bool,
    graph: kirin_ir::DiGraph,
    /// Entry arguments not yet bound (bound on the first `step`).
    pending: Option<Product<V>>,
    /// Remaining schedule, in topological order; `None` until the first step.
    schedule: Option<std::collections::VecDeque<Statement>>,
    yields: Vec<SSAValue>,
    /// Result slots awaiting a pushed child frame's `Finished` completion.
    resume_slots: Option<Product<SSAValue>>,
    _marker: std::marker::PhantomData<fn() -> (V, E)>,
}

impl<V, E> DiGraphFrame<V, E>
where
    V: Clone,
    E: From<InterpreterError>,
{
    /// Walk a digraph as a function body: owns the activation and is the
    /// call's return boundary (completes `Returned` with the yields).
    pub fn function(
        stage: CompileStage,
        index: EnvIndex,
        graph: kirin_ir::DiGraph,
        args: Product<V>,
    ) -> Self {
        Self::with_boundary(stage, index, graph, args, true, true)
    }

    /// Walk a digraph owned by a statement inside another body (pushed by a
    /// dialect frame): borrows the caller's activation and completes
    /// `Finished` with the yields.
    pub fn nested(
        stage: CompileStage,
        index: EnvIndex,
        graph: kirin_ir::DiGraph,
        args: Product<V>,
    ) -> Self {
        Self::with_boundary(stage, index, graph, args, false, false)
    }

    fn with_boundary(
        stage: CompileStage,
        index: EnvIndex,
        graph: kirin_ir::DiGraph,
        args: Product<V>,
        owns_env: bool,
        function_boundary: bool,
    ) -> Self {
        Self {
            stage,
            index,
            owns_env,
            function_boundary,
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
                Err(E::from(InterpreterError::CfgControlFlowInStructuredBody))
            }
            SparseForwardEffect::Yield(_) => Err(E::from(InterpreterError::Custom(
                "yield inside a digraph body (a digraph's outputs are its declared yields)",
            ))),
            SparseForwardEffect::Return(_) => Err(E::from(InterpreterError::Custom(
                "return inside a digraph body",
            ))),
        }
    }

    /// Schedule exhausted: read the yields from the environment and complete.
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
        if self.function_boundary {
            if self.owns_env {
                interp.free_env(self.index)?;
            }
            Ok(FrameEffect::Complete(Completion::Returned(values)))
        } else {
            Ok(FrameEffect::Complete(Completion::Finished(values)))
        }
    }

    /// A child finished without a payload (e.g. a returned call whose results
    /// are already written): resume the schedule.
    pub fn resume_done_into<F>(self) -> FrameEffect<F, Completion<V>>
    where
        F: FrameBuild<V, E>,
    {
        FrameEffect::Continue(F::from_digraph(self))
    }

    /// A child bubbled a completion: a pushed frame `Finished` (bind its
    /// values into the awaiting result slots) or a callee `Returned`.
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
            Completion::Finished(values) => {
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

pub enum StandardFrame<V, E> {
    Body(BodyFrame<V, E>),
    Call(CallFrame<V>),
    DiGraph(DiGraphFrame<V, E>),
}

impl<V, E> FrameBuild<V, E> for StandardFrame<V, E> {
    fn from_body(frame: BodyFrame<V, E>) -> Self {
        StandardFrame::Body(frame)
    }
    fn from_call(frame: CallFrame<V>) -> Self {
        StandardFrame::Call(frame)
    }
    fn from_digraph(frame: DiGraphFrame<V, E>) -> Self {
        StandardFrame::DiGraph(frame)
    }
}

impl<I, V, E> Frame<I> for StandardFrame<V, E>
where
    I: FrameDriver<Value = V, Error = E> + SparseForwardInterp<Frame = StandardFrame<V, E>>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Body(frame) => frame.step_into::<I, Self>(interp),
            StandardFrame::Call(frame) => frame.step_into::<I, Self>(interp),
            StandardFrame::DiGraph(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Body(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardFrame::Call(frame) => frame.resume_done_into::<Self>().map_err(I::Error::from),
            StandardFrame::DiGraph(frame) => Ok(frame.resume_done_into::<Self>()),
        }
    }

    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Body(frame) => frame.resume_into::<I, Self>(completion, interp),
            StandardFrame::Call(frame) => frame.resume_into::<I, Self>(completion, interp),
            StandardFrame::DiGraph(frame) => frame.resume_into::<I, Self>(completion, interp),
        }
    }
}
