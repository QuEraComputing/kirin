//! The **concrete** implementation of the shared [`frame`](crate::frame)
//! protocol.
//!
//! These are the default total frames for [`ConcreteInterpreter`](crate::ConcreteInterpreter):
//! [`ScopeFrame`] (block, region, or hook-driven scope traversal) and
//! [`CallFrame`] (call/return). They implement the shared [`Frame`] trait by
//! consuming the dialect [`ForwardEffect`] and driving a single deterministic path. A
//! compiler/analysis author can define a *custom* total frame enum reusing these
//! via [`FrameBuild`], without forking the engine. The abstract analogue lives
//! in [`abstract_frame`](crate::abstract_frame).

use kirin_ir::{Block, CompileStage, Product, SSAValue, Statement};

use crate::ctx::EngineEnv;
use crate::{
    CallEffect, Callee, EnvIndex, ForwardEffect, Frame, FrameDriver, FrameEffect, InterpreterError,
    Scope, ScopeBody, ScopeHook, ScopeStep,
};

/// Completion payloads produced by the standard concrete frames.
///
/// The only payload that bubbles across frames is a function return; scope
/// yields and block exits are handled inside [`ScopeFrame`] itself. Marked
/// `#[non_exhaustive]` so custom frames can add payloads later without breaking
/// downstream matches.
#[non_exhaustive]
pub enum Completion<V> {
    /// A function returned these values; bubbles to the enclosing
    /// [`CallFrame`], or finishes the run at the root.
    Returned(Product<V>),
}

/// Construction trait letting any total frame enum embed the standard concrete
/// frames.
///
/// The default [`StandardFrame`] implements it trivially; a custom enum
/// implements it to reuse [`ScopeFrame`]/[`CallFrame`] traversal while adding
/// its own variants — the standard frames build successors through this trait,
/// so they are not pinned to [`StandardFrame`].
pub trait FrameBuild<V, E>: Sized {
    fn from_scope(frame: ScopeFrame<V, E>) -> Self;
    fn from_call(frame: CallFrame<V>) -> Self;
}

/// Traversal of one scope body: a block (scf-style), a region's CFG (function
/// bodies), with optional hook-driven re-entry (loops).
pub struct ScopeFrame<V, E> {
    stage: CompileStage,
    env: EnvIndex,
    owns_env: bool,
    function_boundary: bool,
    entry_block: Block,
    entry_args: Product<V>,
    block: Block,
    cursor: Option<Statement>,
    results: Product<SSAValue>,
    hook: Option<Box<dyn ScopeHook<V, E>>>,
}

impl<V, E> ScopeFrame<V, E>
where
    V: Clone,
    E: From<InterpreterError>,
{
    /// Enter a [`Scope`], producing the frame to push. Returns `Ok(None)` for an
    /// [`ScopeBody::Immediate`] scope (its results are written immediately and
    /// no frame is needed).
    pub fn enter<I>(
        interp: &mut I,
        stage: CompileStage,
        env: EnvIndex,
        owns_env: bool,
        function_boundary: bool,
        scope: Scope<V, E>,
    ) -> Result<Option<Self>, E>
    where
        I: FrameDriver<Value = V, Error = E>,
    {
        let entry_block = match scope.body() {
            ScopeBody::Block(block) => block,
            ScopeBody::Region(region) => interp
                .region_entry(stage, region)?
                .ok_or_else(|| E::from(InterpreterError::EmptyRegion))?,
            ScopeBody::Immediate => {
                let Scope { args, results, .. } = scope;
                interp.write_results(env, &results, args)?;
                return Ok(None);
            }
        };
        let Scope {
            args,
            results,
            hook,
            ..
        } = scope;
        interp.bind_block_args(stage, env, entry_block, &args)?;
        let cursor = interp.first_statement(stage, entry_block)?;
        Ok(Some(Self {
            stage,
            env,
            owns_env,
            function_boundary,
            entry_block,
            entry_args: args,
            block: entry_block,
            cursor,
            results,
            hook,
        }))
    }

    /// Execute the next statement and translate its [`ForwardEffect`] into a
    /// [`FrameEffect`] over the total frame type `F`.
    pub fn step_into<I, F>(mut self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E>,
        F: FrameBuild<V, E>,
    {
        let Some(statement) = self.cursor else {
            return Err(E::from(if self.function_boundary {
                InterpreterError::FunctionBodyFellThrough
            } else {
                InterpreterError::BlockFellThrough(self.block)
            }));
        };
        self.cursor = interp.next_statement(self.stage, self.block, statement)?;

        match interp.run_statement(self.stage, statement, self.env)? {
            ForwardEffect::Next => Ok(FrameEffect::Continue(F::from_scope(self))),
            ForwardEffect::Jump(edge) => {
                interp.bind_block_args(self.stage, self.env, edge.target, &edge.args)?;
                self.cursor = interp.first_statement(self.stage, edge.target)?;
                self.block = edge.target;
                Ok(FrameEffect::Continue(F::from_scope(self)))
            }
            ForwardEffect::Branch(_) | ForwardEffect::EnterAny(_) => {
                Err(E::from(InterpreterError::IndeterminateBranch))
            }
            ForwardEffect::Enter(scope) => {
                let stage = self.stage;
                let env = self.env;
                match ScopeFrame::enter(interp, stage, env, false, false, scope)? {
                    Some(child) => Ok(FrameEffect::Push {
                        parent: F::from_scope(self),
                        child: F::from_scope(child),
                    }),
                    // Immediate scope already wrote its results; just continue.
                    None => Ok(FrameEffect::Continue(F::from_scope(self))),
                }
            }
            ForwardEffect::Call(call) => {
                let pending = CallFrame::pending(self.stage, self.env, call);
                Ok(FrameEffect::Push {
                    parent: F::from_scope(self),
                    child: F::from_call(pending),
                })
            }
            ForwardEffect::Yield(values) => self.on_yield::<I, F>(interp, values),
            ForwardEffect::Return(values) => self.finish_return::<I, F>(interp, values),
        }
    }

    fn on_yield<I, F>(
        mut self,
        interp: &mut I,
        values: Product<V>,
    ) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E>,
        F: FrameBuild<V, E>,
    {
        if self.function_boundary {
            return Err(E::from(InterpreterError::Custom(
                "yield reached a function boundary",
            )));
        }
        match self.hook.take() {
            None => {
                interp.write_results(self.env, &self.results, values)?;
                Ok(FrameEffect::Done)
            }
            Some(hook) => {
                let step = {
                    let mut env = EngineEnv {
                        interp: &mut *interp,
                        env: self.env,
                    };
                    hook.on_yield(&self.entry_args, values, &mut env)?
                };
                match step {
                    ScopeStep::Finish(results) => {
                        interp.write_results(self.env, &self.results, results)?;
                        Ok(FrameEffect::Done)
                    }
                    ScopeStep::Repeat { args, hook } => {
                        interp.bind_block_args(self.stage, self.env, self.entry_block, &args)?;
                        self.cursor = interp.first_statement(self.stage, self.entry_block)?;
                        self.block = self.entry_block;
                        self.entry_args = args;
                        self.hook = Some(hook);
                        Ok(FrameEffect::Continue(F::from_scope(self)))
                    }
                    ScopeStep::RepeatOrFinish { .. } => {
                        Err(E::from(InterpreterError::IndeterminateBranch))
                    }
                }
            }
        }
    }

    /// A child frame finished without a payload (its results are already in the
    /// shared env): resume traversal at the advanced cursor.
    pub fn resume_done_into<F>(self) -> FrameEffect<F, Completion<V>>
    where
        F: FrameBuild<V, E>,
    {
        FrameEffect::Continue(F::from_scope(self))
    }

    /// A child bubbled a completion. The standard completion is a function
    /// return, which keeps bubbling (freeing the env at the function boundary).
    pub fn resume_into<I, F>(
        self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, Completion<V>>, E>
    where
        I: FrameDriver<Value = V, Error = E>,
        F: FrameBuild<V, E>,
    {
        match completion {
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
                let scope = interp.enter_function(target.stage, target.body, args, env)?;
                match ScopeFrame::enter(interp, target.stage, env, true, true, scope)? {
                    Some(body) => Ok(FrameEffect::Push {
                        parent: F::from_call(CallFrame::Awaiting {
                            caller_env,
                            results,
                        }),
                        child: F::from_scope(body),
                    }),
                    None => {
                        interp.free_env(env)?;
                        Err(I::Error::from(InterpreterError::FunctionBodyFellThrough))
                    }
                }
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
            (CallFrame::Pending { .. }, _) => Err(I::Error::from(InterpreterError::Custom(
                "call frame resumed before dispatch",
            ))),
        }
    }
}

/// The default total concrete frame enum: standard concrete traversal.
pub enum StandardFrame<V, E> {
    Scope(ScopeFrame<V, E>),
    Call(CallFrame<V>),
}

impl<V, E> FrameBuild<V, E> for StandardFrame<V, E> {
    fn from_scope(frame: ScopeFrame<V, E>) -> Self {
        StandardFrame::Scope(frame)
    }
    fn from_call(frame: CallFrame<V>) -> Self {
        StandardFrame::Call(frame)
    }
}

impl<I> Frame<I> for StandardFrame<I::Value, I::Error>
where
    I: FrameDriver,
    I::Value: Clone,
    I::Error: From<InterpreterError>,
{
    type Completion = Completion<I::Value>;

    fn step(self, interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Scope(frame) => frame.step_into::<I, Self>(interp),
            StandardFrame::Call(frame) => frame.step_into::<I, Self>(interp),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Scope(frame) => Ok(frame.resume_done_into::<Self>()),
            StandardFrame::Call(frame) => frame.resume_done_into::<Self>().map_err(I::Error::from),
        }
    }

    fn resume(
        self,
        completion: Self::Completion,
        interp: &mut I,
    ) -> Result<FrameEffect<Self, Self::Completion>, I::Error> {
        match self {
            StandardFrame::Scope(frame) => frame.resume_into::<I, Self>(completion, interp),
            StandardFrame::Call(frame) => frame.resume_into::<I, Self>(completion, interp),
        }
    }
}
