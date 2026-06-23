//! The **concrete** implementation of the shared [`frame`](crate::frame)
//! protocol.
//!
//! These are the default total frames for [`ConcreteInterpreter`](crate::ConcreteInterpreter):
//! [`BodyFrame`] (walks a function-body CFG region or a single body block) and
//! [`CallFrame`] (call/return). They implement the shared [`Frame`] trait by
//! consuming the dialect [`ForwardEffect`] and driving a single deterministic
//! path. Structured-control dialects do not get a framework "scope": they push
//! a frame **they own** through [`ForwardEffect::Push`] (that frame may build a
//! [`BodyFrame`] to walk a chosen body — a reusable building block, not
//! framework-owned structured semantics). A language that combines such a
//! dialect defines its own total frame enum embedding [`BodyFrame`]/[`CallFrame`]
//! via [`FrameBuild`] plus its dialect frames. The abstract analogue lives in
//! [`abstract_frame`](crate::abstract_frame).

use kirin_ir::{Block, CompileStage, Product, Region, SSAValue, Statement};

use crate::{
    CallEffect, Callee, EnvIndex, ForwardEffect, ForwardInterp, Frame, FrameDriver, FrameEffect,
    InterpreterError,
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
}

/// Traversal of one body: a function-body CFG region (multi-block, with jumps)
/// or a single body block (scf-style, terminated by a yield).
pub struct BodyFrame<V, E> {
    stage: CompileStage,
    env: EnvIndex,
    owns_env: bool,
    function_boundary: bool,
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
    /// Walk a function body: start at the entry block of `region`, binding
    /// `args` to its parameters. Owns the activation and is the return boundary.
    pub fn function<I>(
        interp: &mut I,
        stage: CompileStage,
        env: EnvIndex,
        region: Region,
        args: Product<V>,
    ) -> Result<Self, E>
    where
        I: FrameDriver<Value = V, Error = E>,
    {
        let entry = interp
            .region_entry(stage, region)?
            .ok_or_else(|| E::from(InterpreterError::EmptyRegion))?;
        Self::start(interp, stage, env, entry, args, true, true)
    }

    /// A single body block (scf-style), to bind `args` to its parameters on the
    /// first step. Borrows the caller's activation and is not a return boundary.
    /// Pure construction — needs no engine access.
    pub fn block(stage: CompileStage, env: EnvIndex, block: Block, args: Product<V>) -> Self {
        Self {
            stage,
            env,
            owns_env: false,
            function_boundary: false,
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
        env: EnvIndex,
        block: Block,
        args: Product<V>,
        owns_env: bool,
        function_boundary: bool,
    ) -> Result<Self, E>
    where
        I: FrameDriver<Value = V, Error = E>,
    {
        interp.bind_block_args(stage, env, block, &args)?;
        let cursor = interp.first_statement(stage, block)?;
        Ok(Self {
            stage,
            env,
            owns_env,
            function_boundary,
            block,
            cursor,
            pending: None,
            resume_slots: None,
            _marker: std::marker::PhantomData,
        })
    }

    /// Execute the next statement and translate its [`ForwardEffect`] into a
    /// [`FrameEffect`] over the total frame type `F`.
    pub fn step_into<I, F>(mut self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E> + ForwardInterp<Frame = F>,
        F: FrameBuild<V, E>,
    {
        // Bind entry arguments lazily on the first step (a dialect-built body
        // frame carries them unbound).
        if let Some(args) = self.pending.take() {
            interp.bind_block_args(self.stage, self.env, self.block, &args)?;
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

        match interp.run_statement(self.stage, statement, self.env)? {
            ForwardEffect::Next => Ok(FrameEffect::Continue(F::from_body(self))),
            ForwardEffect::Jump(edge) => {
                interp.bind_block_args(self.stage, self.env, edge.target, &edge.args)?;
                self.cursor = interp.first_statement(self.stage, edge.target)?;
                self.block = edge.target;
                Ok(FrameEffect::Continue(F::from_body(self)))
            }
            ForwardEffect::Branch(_) => Err(E::from(InterpreterError::IndeterminateBranch)),
            ForwardEffect::Push { frame, results } => {
                self.resume_slots = Some(results);
                Ok(FrameEffect::Push {
                    parent: F::from_body(self),
                    child: frame,
                })
            }
            ForwardEffect::Call(call) => {
                let pending = CallFrame::pending(self.stage, self.env, call);
                Ok(FrameEffect::Push {
                    parent: F::from_body(self),
                    child: F::from_call(pending),
                })
            }
            ForwardEffect::Yield(values) => {
                if self.function_boundary {
                    return Err(E::from(InterpreterError::Custom(
                        "yield reached a function boundary",
                    )));
                }
                Ok(FrameEffect::Complete(Completion::Finished(values)))
            }
            ForwardEffect::Return(values) => self.finish_return::<I, F>(interp, values),
        }
    }

    /// A child finished without a payload (its results are already in the
    /// shared env, e.g. a returned call): resume at the advanced cursor.
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
                interp.write_results(self.env, &slots, values)?;
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
            interp.free_env(self.env)?;
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
                let env = interp.alloc_env();
                let body = interp.enter_function(target.stage, target.body, args, env)?;
                let frame = BodyFrame::function(interp, target.stage, env, body.region, body.args)?;
                Ok(FrameEffect::Push {
                    parent: F::from_call(CallFrame::Awaiting {
                        caller_env,
                        results,
                    }),
                    child: F::from_body(frame),
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
pub enum StandardFrame<V, E> {
    Body(BodyFrame<V, E>),
    Call(CallFrame<V>),
}

impl<V, E> FrameBuild<V, E> for StandardFrame<V, E> {
    fn from_body(frame: BodyFrame<V, E>) -> Self {
        StandardFrame::Body(frame)
    }
    fn from_call(frame: CallFrame<V>) -> Self {
        StandardFrame::Call(frame)
    }
}

impl<I, V, E> Frame<I> for StandardFrame<V, E>
where
    I: FrameDriver<Value = V, Error = E> + ForwardInterp<Frame = StandardFrame<V, E>>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Body(frame) => frame.step_into::<I, Self>(interp),
            StandardFrame::Call(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Body(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardFrame::Call(frame) => frame.resume_done_into::<Self>().map_err(I::Error::from),
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
        }
    }
}
