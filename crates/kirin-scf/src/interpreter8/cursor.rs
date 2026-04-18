use std::marker::PhantomData;

use kirin::prelude::Dialect;
use kirin::prelude::{Block, CompileStage, ResultValue};
use kirin_interpreter::ProductValue;
use kirin_interpreter_8::algebra::Lift;
use kirin_interpreter_8::control::{Control, CursorExt};
use kirin_interpreter_8::cursor::{BlockCursor, Execute};
use kirin_interpreter_8::env::ConcreteEnv;
use kirin_interpreter_8::error::InterpreterError;

use crate::ForLoopValue;

// ---------------------------------------------------------------------------
// SCFCursor — composed cursor for scf dialect within language L
//
// TODO: #[derive(ComposedCursor)] will generate this for any language that
// includes SCF. Written manually until the derive is implemented.
// ---------------------------------------------------------------------------

/// Composed cursor for the SCF dialect within language `L`.
pub enum SCFCursor<V, L: Dialect> {
    If(IfCursor<V, L>),
    For(ForCursor<V, L>),
}

impl<D, V, L> Execute<D> for SCFCursor<V, L>
where
    L: Dialect,
    V: Clone,
    D: ConcreteEnv<Value = V>,
    IfCursor<V, L>: Execute<D>,
    ForCursor<V, L>: Execute<D>,
{
    fn execute(&mut self, domain: &mut D) -> Result<Control<V, D::Ext>, D::Error> {
        match self {
            SCFCursor::If(c) => c.execute(domain),
            SCFCursor::For(c) => c.execute(domain),
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
/// Phase 2 (CollectYield): collects the pending yield, writes results, produces Pop.
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

impl<D, V, L> Execute<D> for IfCursor<V, L>
where
    V: Clone + ProductValue + 'static,
    L: Dialect,
    D: ConcreteEnv<Value = V>,
    BlockCursor<V, L>: Lift<D::Cursor>,
    D::Ext: From<CursorExt<D::Cursor>>,
    D::Error: From<InterpreterError>,
{
    fn execute(&mut self, domain: &mut D) -> Result<Control<V, D::Ext>, D::Error> {
        match std::mem::replace(&mut self.phase, IfPhase::Done(PhantomData)) {
            IfPhase::PushBody { body, results } => {
                let cursor = BlockCursor::<V, L>::new(body, self.body_stage, vec![]);
                let lifted: D::Cursor = cursor.lift();
                self.phase = IfPhase::CollectYield { results };
                Ok(Control::Ext(D::Ext::from(CursorExt::Push(lifted))))
            }
            IfPhase::CollectYield { results } => {
                if let Some(product) = domain.take_pending_yield() {
                    domain.write_product(&results, product)?;
                }
                Ok(Control::Ext(D::Ext::from(CursorExt::Pop)))
            }
            IfPhase::Done(_) => Err(D::Error::from(InterpreterError::UnhandledEffect(
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

impl<D, V, L> Execute<D> for ForCursor<V, L>
where
    V: Clone + ProductValue + ForLoopValue + 'static,
    L: Dialect,
    D: ConcreteEnv<Value = V>,
    BlockCursor<V, L>: Lift<D::Cursor>,
    D::Ext: From<CursorExt<D::Cursor>>,
    D::Error: From<InterpreterError>,
{
    fn execute(&mut self, domain: &mut D) -> Result<Control<V, D::Ext>, D::Error> {
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
                    domain.write_product(&results, carried)?;
                    return Ok(Control::Ext(D::Ext::from(CursorExt::Pop)));
                }

                let mut block_args = Vec::with_capacity(1 + init_arg_count);
                block_args.push(iv.clone());
                if let Some(product) = carried.as_product() {
                    block_args.extend(product.iter().cloned());
                } else if init_arg_count > 0 {
                    block_args.push(carried);
                }

                let cursor = BlockCursor::<V, L>::new(body, self.body_stage, block_args);
                let lifted: D::Cursor = cursor.lift();
                self.phase = ForPhase::CollectAndStep {
                    iv,
                    end,
                    step,
                    body,
                    init_arg_count,
                    results,
                };
                Ok(Control::Ext(D::Ext::from(CursorExt::Push(lifted))))
            }
            ForPhase::CollectAndStep {
                iv,
                end,
                step,
                body,
                init_arg_count,
                results,
            } => {
                let carried = domain.take_pending_yield().ok_or_else(|| {
                    D::Error::from(InterpreterError::UnhandledEffect(
                        "scf.for body did not yield a value".into(),
                    ))
                })?;

                let next_iv = iv.loop_step(&step).ok_or_else(|| {
                    D::Error::from(InterpreterError::UnhandledEffect(
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
                Ok(Control::Advance)
            }
            ForPhase::Done(_) => Err(D::Error::from(InterpreterError::UnhandledEffect(
                "ForCursor executed after completion".into(),
            ))),
        }
    }
}
