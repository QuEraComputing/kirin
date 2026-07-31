use kirin_ir::{CompileStage, Product, SSAValue};

use crate::{
    Body, CallEffect, Callee, EnvIndex, Frame, FrameDriver, FrameEffect, InterpreterError,
};

use super::{BlockFrame, CFGFrame, Completion, DiGraphFrame, FrameBuild, UnGraphEntry};

/// The function-call boundary frame: interpreter runtime bookkeeping, not a
/// function dialect operation and not the callable itself.
///
/// A `CallFrame` owns the whole activation lifecycle that representation
/// walkers deliberately don't:
///
/// 1. resolve the callee through the engine's [`Linker`](crate::Linker);
/// 2. allocate the callee activation;
/// 3. ask [`FunctionEntry`](crate::FunctionEntry) for the callable body
///    descriptor ([`CallableBody`](crate::CallableBody));
/// 4. select the entry frame for the closed [`Body`] variant —
///    `CFG` → [`CFGFrame`], `Block` → [`BlockFrame`],
///    `DiGraph` → [`DiGraphFrame`], `UnGraph` → the dialect/compiler policy
///    ([`FrameBuild::from_ungraph_entry`]);
/// 5. suspend while the callee frame runs;
/// 6. validate the callee's completion kind ([`Returned`](Completion::Returned)
///    or a graph's natural [`Finished`](Completion::Finished) are returns; a
///    structured [`Yielded`](Completion::Yielded) is an error);
/// 7. free the callee activation exactly once;
/// 8. deliver the returned values — into the caller's result slots for a
///    nested call, or as the run's completion for a root call
///    ([`ConcreteInterpreter::call`](crate::ConcreteInterpreter::call) pushes
///    a [`CallFrame::root`], so root and nested calls share this one
///    boundary implementation).
pub struct CallFrame<V> {
    state: CallState<V>,
}

enum CallState<V> {
    /// Not yet dispatched: resolve the callee and enter its body.
    Pending {
        resolve_stage: CompileStage,
        callee: Callee,
        args: Product<V>,
        dest: CallDest,
    },
    /// Dispatched: the callee frame is running. Holds the callee activation
    /// so the boundary frees it exactly once on completion.
    Awaiting {
        callee_env: EnvIndex,
        dest: CallDest,
    },
}

/// Where a finished call delivers its returned values.
enum CallDest {
    /// Write into result slots of the calling activation and resume the
    /// caller.
    Caller {
        env: EnvIndex,
        results: Product<SSAValue>,
    },
    /// A root call: complete the frame stack with the values.
    Root,
}

impl<V> CallFrame<V>
where
    V: Clone,
{
    /// A call issued by a statement ([`SparseForwardEffect::Call`](crate::SparseForwardEffect::Call)):
    /// returned values land in `call.results` of the caller's activation.
    pub fn pending(scope_stage: CompileStage, caller_env: EnvIndex, call: CallEffect<V>) -> Self {
        CallFrame {
            state: CallState::Pending {
                resolve_stage: call.stage.unwrap_or(scope_stage),
                callee: call.callee,
                args: call.args,
                dest: CallDest::Caller {
                    env: caller_env,
                    results: call.results,
                },
            },
        }
    }

    /// A root call (no calling activation): the returned values complete the
    /// frame stack.
    pub fn root(stage: CompileStage, callee: Callee, args: Product<V>) -> Self {
        CallFrame {
            state: CallState::Pending {
                resolve_stage: stage,
                callee,
                args,
                dest: CallDest::Root,
            },
        }
    }
}

impl<I, F, V, E> Frame<I, F> for CallFrame<V>
where
    I: FrameDriver<Value = V, Error = E>,
    F: FrameBuild<V, E>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = Completion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        match self.state {
            CallState::Pending {
                resolve_stage,
                callee,
                args,
                dest,
            } => {
                let target = interp.resolve_call(resolve_stage, &callee)?;
                let index = interp.alloc_env();
                let entry = interp.enter_function(target.stage, target.body, args, index)?;
                // The closed `Body` enum is the framework's supported body
                // vocabulary, so this match is intentionally exhaustive;
                // only the `UnGraph` arm delegates to a language policy.
                let child = match entry.body {
                    Body::CFG(cfg) => {
                        F::from_cfg(CFGFrame::new(target.stage, index, cfg, entry.args))
                    }
                    Body::Block(block) => {
                        F::from_block(BlockFrame::new(target.stage, index, block, entry.args))
                    }
                    Body::DiGraph(graph) => {
                        F::from_digraph(DiGraphFrame::new(target.stage, index, graph, entry.args))
                    }
                    Body::UnGraph(graph) => F::from_ungraph_entry(UnGraphEntry {
                        stage: target.stage,
                        index,
                        graph,
                        args: entry.args,
                    })?,
                };
                Ok(FrameEffect::Push {
                    parent: F::from_call(CallFrame {
                        state: CallState::Awaiting {
                            callee_env: index,
                            dest,
                        },
                    }),
                    child,
                })
            }
            CallState::Awaiting { .. } => Err(E::from(InterpreterError::Custom(
                "call frame stepped while awaiting a return",
            ))),
        }
    }

    fn resume_done_into(self, _interp: &mut I) -> Result<FrameEffect<F, Completion<V>>, E> {
        Err(E::from(InterpreterError::Custom(
            "call frame resumed without a return",
        )))
    }

    /// The callee completed: validate the completion kind, free the callee
    /// activation exactly once, and deliver the returned values.
    fn resume_into(
        self,
        completion: Completion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, Completion<V>>, E> {
        let CallState::Awaiting { callee_env, dest } = self.state else {
            return Err(E::from(InterpreterError::Custom(
                "call frame resumed before dispatch",
            )));
        };
        let values = match completion {
            // An explicit `Return`, or a graph body's natural completion
            // (a callable DiGraph's outputs are the call's returned values).
            Completion::Returned(values) | Completion::Finished(values) => values,
            Completion::Yielded(_) => {
                return Err(E::from(InterpreterError::Custom(
                    "structured yield reached a function-call boundary (a callable body must exit with return)",
                )));
            }
        };
        interp.free_env(callee_env)?;
        match dest {
            CallDest::Caller { env, results } => {
                interp.write_results(env, &results, values)?;
                Ok(FrameEffect::Done)
            }
            CallDest::Root => Ok(FrameEffect::Complete(Completion::Returned(values))),
        }
    }
}
