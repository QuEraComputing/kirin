use std::marker::PhantomData;

use kirin::prelude::{Block, CompileStage, ResultValue};
use kirin_interpreter::ProductValue;
use kirin_interpreter_5::concrete::ConcreteDomain;
use kirin_interpreter_5::cursor::{Boxed, Execute};
use kirin_interpreter_5::effect::ControlFlow;
use kirin_interpreter_5::env::Env;
use kirin_interpreter_5::error::InterpreterError;

use crate::ForLoopValue;

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

pub struct IfCursor<V> {
    phase: IfPhase<V>,
    body_stage: CompileStage,
}

impl<V> IfCursor<V> {
    pub fn new(body: Block, results: Vec<ResultValue>, body_stage: CompileStage) -> Self {
        Self {
            phase: IfPhase::PushBody { body, results },
            body_stage,
        }
    }
}

impl<D, V> Execute<D> for IfCursor<V>
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone + ProductValue,
    D::Error: From<InterpreterError>,
{
    fn execute(&mut self, domain: &mut D) -> Result<ControlFlow<V, Boxed<D>>, D::Error> {
        match std::mem::replace(&mut self.phase, IfPhase::Done(PhantomData)) {
            IfPhase::PushBody { body, results } => {
                let cursor = domain.make_block_cursor(body, self.body_stage, vec![])?;
                self.phase = IfPhase::CollectYield { results };
                Ok(ControlFlow::Push(cursor))
            }
            IfPhase::CollectYield { results } => {
                if let Some(product) = domain.take_pending_yield() {
                    domain.write_product(&results, product)?;
                }
                Ok(ControlFlow::Pop)
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

pub struct ForCursor<V> {
    phase: ForPhase<V>,
    body_stage: CompileStage,
}

impl<V> ForCursor<V> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        iv: V,
        end: V,
        step: V,
        carried: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
        body_stage: CompileStage,
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
        }
    }
}

impl<D, V> Execute<D> for ForCursor<V>
where
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    V: Clone + ProductValue + ForLoopValue,
    D::Error: From<InterpreterError>,
{
    fn execute(&mut self, domain: &mut D) -> Result<ControlFlow<V, Boxed<D>>, D::Error> {
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
                    domain.write_product(&results, carried)?;
                    return Ok(ControlFlow::Pop);
                }

                let mut block_args = Vec::with_capacity(1 + init_arg_count);
                block_args.push(iv.clone());
                if let Some(product) = carried.as_product() {
                    block_args.extend(product.iter().cloned());
                } else if init_arg_count > 0 {
                    block_args.push(carried);
                }

                let cursor = domain.make_block_cursor(body, self.body_stage, block_args)?;
                self.phase = ForPhase::CollectAndStep {
                    iv,
                    end,
                    step,
                    body,
                    init_arg_count,
                    results,
                };
                Ok(ControlFlow::Push(cursor))
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
                        "scf.for: induction variable overflow during loop step".into(),
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
                Ok(ControlFlow::Advance)
            }
            ForPhase::Done(_) => Err(D::Error::from(InterpreterError::UnhandledEffect(
                "ForCursor executed after completion".into(),
            ))),
        }
    }
}
