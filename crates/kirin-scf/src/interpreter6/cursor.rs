use std::marker::PhantomData;

use kirin::prelude::Dialect;
use kirin::prelude::{Block, CompileStage, ResultValue};
use kirin_interpreter::ProductValue;
use kirin_interpreter_6::concrete::ConcreteDomain;
use kirin_interpreter_6::core::Core;
use kirin_interpreter_6::cursor::{BlockCursor, Execute};
use kirin_interpreter_6::env::Env;
use kirin_interpreter_6::error::InterpreterError;
use kirin_interpreter_6::lift::{Lift, Project};

use crate::ForLoopValue;

// ---------------------------------------------------------------------------
// SCFCursor — composed cursor for scf dialect within language L
//
// #[derive(ComposedCursor)] generates this for any language that includes SCF.
// Written manually until the derive is implemented.
// ---------------------------------------------------------------------------

/// Composed cursor for the SCF dialect within language `L`.
///
/// Dispatches to `IfCursor<V, L>` or `ForCursor<V, L>` — both of which create
/// `BlockCursor<V, L>` for body block execution.
pub enum SCFCursor<V, L: Dialect> {
    If(IfCursor<V, L>),
    For(ForCursor<V, L>),
}

/// Execute<E> for SCFCursor: dispatch to the appropriate inner cursor.
///
/// Derived by #[derive(ComposedCursor)] — written manually here.
impl<E, V, L> Execute<E> for SCFCursor<V, L>
where
    L: Dialect,
    V: Clone,
    E: Env<Value = V>,
    IfCursor<V, L>: Execute<E>,
    ForCursor<V, L>: Execute<E>,
{
    fn execute(&mut self, env: &mut E) -> Result<E::Effect, E::Error> {
        match self {
            SCFCursor::If(c) => c.execute(env),
            SCFCursor::For(c) => c.execute(env),
        }
    }
}

// ---------------------------------------------------------------------------
// IfCursor — two-phase inline execution for scf.if
// ---------------------------------------------------------------------------

enum IfPhase<V> {
    PushBody {
        body: Block,
        results: Vec<ResultValue>,
    },
    CollectYield {
        results: Vec<ResultValue>,
    },
    Done(PhantomData<V>),
}

/// Two-phase cursor for `scf.if`.
///
/// Phase 1 (PushBody): pushes a `BlockCursor<V, L>` for the chosen branch body.
/// Phase 2 (CollectYield): collects the pending yield, writes results, pops.
///
/// `L` is the containing language — needed to create `BlockCursor<V, L>`.
pub struct IfCursor<V, L: Dialect> {
    phase: IfPhase<V>,
    body_stage: CompileStage,
    _phantom: PhantomData<L>,
}

impl<V, L: Dialect> IfCursor<V, L> {
    pub fn new(body: Block, results: Vec<ResultValue>, body_stage: CompileStage) -> Self {
        Self {
            phase: IfPhase::PushBody { body, results },
            body_stage,
            _phantom: PhantomData,
        }
    }
}

impl<E, V, L> Execute<E> for IfCursor<V, L>
where
    V: Clone + ProductValue + 'static,
    L: Dialect,
    E: ConcreteDomain<Value = V>,
    // C = E::Cursor must be able to hold a BlockCursor<V, L>.
    E::Cursor: Lift<BlockCursor<V, L>>,
    // The effect must support Core (for Push and Pop).
    E::Effect: Lift<Core<V, E::Cursor>> + Project<Core<V, E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    fn execute(&mut self, env: &mut E) -> Result<E::Effect, E::Error> {
        match std::mem::replace(&mut self.phase, IfPhase::Done(PhantomData)) {
            IfPhase::PushBody { body, results } => {
                let cursor = BlockCursor::<V, L>::new(body, self.body_stage, vec![]);
                let lifted: E::Cursor = E::Cursor::lift(cursor);
                self.phase = IfPhase::CollectYield { results };
                Ok(E::Effect::lift(Core::Push(lifted)))
            }
            IfPhase::CollectYield { results } => {
                if let Some(product) = env.take_pending_yield() {
                    env.write_product(&results, product)?;
                }
                Ok(E::Effect::lift(Core::Pop))
            }
            IfPhase::Done(_) => Err(E::Error::from(InterpreterError::UnhandledEffect(
                "IfCursor executed after completion".into(),
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// ForCursor — multi-phase inline execution for scf.for
// ---------------------------------------------------------------------------

enum ForPhase<V> {
    CheckAndPush {
        iv: V,
        end: V,
        step: V,
        carried: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
    },
    CollectAndStep {
        iv: V,
        end: V,
        step: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
    },
    Done(PhantomData<V>),
}

/// Multi-phase cursor for `scf.for`.
///
/// Cycles between CheckAndPush (test loop condition, push body block cursor) and
/// CollectAndStep (collect yield, advance IV) until the condition fails, then
/// writes the final carried values and pops.
///
/// `L` is the containing language — needed to create `BlockCursor<V, L>`.
pub struct ForCursor<V, L: Dialect> {
    phase: ForPhase<V>,
    body_stage: CompileStage,
    _phantom: PhantomData<L>,
}

#[bon::bon]
impl<V, L: Dialect> ForCursor<V, L> {
    #[builder]
    pub fn new(
        iv: V,
        end: V,
        step: V,
        carried: V,
        body: Block,
        body_stage: CompileStage,
        #[builder(default)] init_arg_count: usize,
        #[builder(default)] results: Vec<ResultValue>,
    ) -> Self {
        Self {
            phase: ForPhase::CheckAndPush {
                iv,
                end,
                step,
                carried,
                body,
                init_arg_count,
                results,
            },
            body_stage,
            _phantom: PhantomData,
        }
    }
}

impl<E, V, L> Execute<E> for ForCursor<V, L>
where
    V: Clone + ProductValue + ForLoopValue + 'static,
    L: Dialect,
    E: ConcreteDomain<Value = V>,
    E::Cursor: Lift<BlockCursor<V, L>>,
    E::Effect: Lift<Core<V, E::Cursor>> + Project<Core<V, E::Cursor>>,
    E::Error: From<InterpreterError>,
{
    fn execute(&mut self, env: &mut E) -> Result<E::Effect, E::Error> {
        match std::mem::replace(&mut self.phase, ForPhase::Done(PhantomData)) {
            ForPhase::CheckAndPush {
                iv,
                end,
                step,
                carried,
                body,
                init_arg_count,
                results,
            } => {
                if iv.loop_condition(&end) != Some(true) {
                    // Loop finished: write carried values to results and pop.
                    env.write_product(&results, carried)?;
                    return Ok(E::Effect::lift(Core::Pop));
                }

                let mut block_args = Vec::with_capacity(1 + init_arg_count);
                block_args.push(iv.clone());
                if let Some(product) = carried.as_product() {
                    block_args.extend(product.iter().cloned());
                } else if init_arg_count > 0 {
                    block_args.push(carried);
                }

                let cursor = BlockCursor::<V, L>::new(body, self.body_stage, block_args);
                let lifted: E::Cursor = E::Cursor::lift(cursor);
                self.phase = ForPhase::CollectAndStep {
                    iv,
                    end,
                    step,
                    body,
                    init_arg_count,
                    results,
                };
                Ok(E::Effect::lift(Core::Push(lifted)))
            }
            ForPhase::CollectAndStep {
                iv,
                end,
                step,
                body,
                init_arg_count,
                results,
            } => {
                let carried = env.take_pending_yield().ok_or_else(|| {
                    E::Error::from(InterpreterError::UnhandledEffect(
                        "scf.for body did not yield a value".into(),
                    ))
                })?;

                let next_iv = iv.loop_step(&step).ok_or_else(|| {
                    E::Error::from(InterpreterError::UnhandledEffect(
                        "scf.for: induction variable overflow".into(),
                    ))
                })?;

                self.phase = ForPhase::CheckAndPush {
                    iv: next_iv,
                    end,
                    step,
                    carried,
                    body,
                    init_arg_count,
                    results,
                };
                Ok(E::Effect::lift(Core::Advance))
            }
            ForPhase::Done(_) => Err(E::Error::from(InterpreterError::UnhandledEffect(
                "ForCursor executed after completion".into(),
            ))),
        }
    }
}
