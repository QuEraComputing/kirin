use std::marker::PhantomData;

use kirin::prelude::{Block, CompileStage, Dialect, ResultValue};
use kirin_interpreter::ProductValue;
use kirin_interpreter_16::algebra::Lift;
use kirin_interpreter_16::control::{Control, CursorExt};
use kirin_interpreter_16::cursor::{AbstractBlockCursor, BlockCursor};
use kirin_interpreter_16::env::{AbstractEnv, AbstractMode, ConcreteMode, Env};
use kirin_interpreter_16::error::InterpreterError;
use kirin_interpreter_16::execute::Execute;

use crate::ForLoopValue;

// ---------------------------------------------------------------------------
// IfCursor — concrete mode
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

impl<E, C, V, L> Execute<E> for IfCursor<V, L>
where
    V: Clone + ProductValue,
    L: Dialect,
    E: Env<Mode = ConcreteMode<C>, Value = V, Ext = CursorExt<C>>,
    BlockCursor<V, L>: Lift<C>,
    E::Error: From<InterpreterError>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<C>>, E::Error> {
        match std::mem::replace(&mut self.phase, IfPhase::Done(PhantomData)) {
            IfPhase::PushBody { body, results } => {
                let cursor = BlockCursor::<V, L>::new(body, self.body_stage, vec![]);
                let lifted: C = cursor.lift();
                self.phase = IfPhase::CollectYield { results };
                Ok(Control::Ext(CursorExt::Push(lifted)))
            }
            IfPhase::CollectYield { results } => {
                if let Some(product) = inbox {
                    env.write_results(&results, product)?;
                }
                Ok(Control::Ext(CursorExt::Pop))
            }
            IfPhase::Done(_) => Err(E::Error::from(InterpreterError::UnhandledEffect(
                "IfCursor executed after completion".into(),
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// ForCursor — concrete mode
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

pub struct ForCursor<V, L: Dialect> {
    phase: ForPhase<V>,
    body_stage: CompileStage,
    _phantom: PhantomData<L>,
}

impl<V: Clone, L: Dialect> ForCursor<V, L> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        iv: V,
        end: V,
        step: V,
        carried: V,
        body: Block,
        body_stage: CompileStage,
        init_arg_count: usize,
        results: Vec<ResultValue>,
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

impl<E, C, V, L> Execute<E> for ForCursor<V, L>
where
    V: Clone + ProductValue + ForLoopValue,
    L: Dialect,
    E: Env<Mode = ConcreteMode<C>, Value = V, Ext = CursorExt<C>>,
    BlockCursor<V, L>: Lift<C>,
    E::Error: From<InterpreterError>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<C>>, E::Error> {
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
                    env.write_results(&results, carried)?;
                    return Ok(Control::Ext(CursorExt::Pop));
                }

                let mut block_args = Vec::with_capacity(1 + init_arg_count);
                block_args.push(iv.clone());
                if let Some(product) = carried.as_product() {
                    block_args.extend(product.iter().cloned());
                } else if init_arg_count > 0 {
                    block_args.push(carried);
                }

                let cursor = BlockCursor::<V, L>::new(body, self.body_stage, block_args);
                let lifted: C = cursor.lift();
                self.phase = ForPhase::CollectAndStep {
                    iv,
                    end,
                    step,
                    body,
                    init_arg_count,
                    results,
                };
                Ok(Control::Ext(CursorExt::Push(lifted)))
            }
            ForPhase::CollectAndStep {
                iv,
                end,
                step,
                body,
                init_arg_count,
                results,
            } => {
                let carried = inbox.ok_or_else(|| {
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
                Ok(Control::Advance)
            }
            ForPhase::Done(_) => Err(E::Error::from(InterpreterError::UnhandledEffect(
                "ForCursor executed after completion".into(),
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// AbstractIfCursor — abstract mode
// ---------------------------------------------------------------------------

enum AbstractIfPhase<V> {
    PushThen {
        then_body: Block,
        else_body: Block,
        results: Vec<ResultValue>,
    },
    WaitThen {
        else_body: Block,
        results: Vec<ResultValue>,
    },
    PushElse {
        then_result: Option<V>,
        else_body: Block,
        results: Vec<ResultValue>,
    },
    WaitElse {
        then_result: Option<V>,
        results: Vec<ResultValue>,
    },
    Done(PhantomData<V>),
}

pub struct AbstractIfCursor<V, L: Dialect> {
    phase: AbstractIfPhase<V>,
    body_stage: CompileStage,
    _phantom: PhantomData<L>,
}

impl<V: Clone, L: Dialect> AbstractIfCursor<V, L> {
    pub fn new(
        then_body: Block,
        else_body: Block,
        results: Vec<ResultValue>,
        body_stage: CompileStage,
    ) -> Self {
        Self {
            phase: AbstractIfPhase::PushThen {
                then_body,
                else_body,
                results,
            },
            body_stage,
            _phantom: PhantomData,
        }
    }
}

impl<E, C, V, L> Execute<E> for AbstractIfCursor<V, L>
where
    V: Clone + ProductValue + kirin_interpreter::AbstractValue,
    L: Dialect,
    E: AbstractEnv<Value = V, Ext = CursorExt<C>>,
    E: Env<Mode = AbstractMode<C>>,
    AbstractBlockCursor<V, L>: Lift<C>,
    E::Error: From<InterpreterError>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<C>>, E::Error> {
        match std::mem::replace(&mut self.phase, AbstractIfPhase::Done(PhantomData)) {
            AbstractIfPhase::PushThen {
                then_body,
                else_body,
                results,
            } => {
                let cursor = AbstractBlockCursor::<V, L>::new(then_body, self.body_stage, vec![]);
                self.phase = AbstractIfPhase::WaitThen { else_body, results };
                Ok(Control::Ext(CursorExt::Push(cursor.lift())))
            }
            AbstractIfPhase::WaitThen { else_body, results } => {
                self.phase = AbstractIfPhase::PushElse {
                    then_result: inbox,
                    else_body,
                    results,
                };
                Ok(Control::Advance)
            }
            AbstractIfPhase::PushElse {
                then_result,
                else_body,
                results,
            } => {
                let cursor = AbstractBlockCursor::<V, L>::new(else_body, self.body_stage, vec![]);
                self.phase = AbstractIfPhase::WaitElse {
                    then_result,
                    results,
                };
                Ok(Control::Ext(CursorExt::Push(cursor.lift())))
            }
            AbstractIfPhase::WaitElse {
                then_result,
                results,
            } => {
                let joined = match (then_result, inbox) {
                    (Some(a), Some(b)) => Some(a.join(&b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };
                if let Some(v) = joined {
                    env.write_results(&results, v)?;
                }
                Ok(Control::Ext(CursorExt::Pop))
            }
            AbstractIfPhase::Done(_) => Err(E::Error::from(InterpreterError::UnhandledEffect(
                "AbstractIfCursor executed after completion".into(),
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// AbstractForCursor — abstract mode
//
// WaitBody stores `prev_carried` for early convergence via is_subseteq check
// before budget exhaustion.
// ---------------------------------------------------------------------------

enum AbstractForPhase<V> {
    PushBody {
        carried: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
        iterations: usize,
    },
    WaitBody {
        prev_carried: V,
        body: Block,
        init_arg_count: usize,
        results: Vec<ResultValue>,
        iterations: usize,
    },
    Done(PhantomData<V>),
}

pub struct AbstractForCursor<V, L: Dialect> {
    phase: AbstractForPhase<V>,
    body_stage: CompileStage,
    max_iterations: usize,
    _phantom: PhantomData<L>,
}

impl<V: Clone, L: Dialect> AbstractForCursor<V, L> {
    pub fn new(
        carried: V,
        body: Block,
        body_stage: CompileStage,
        init_arg_count: usize,
        results: Vec<ResultValue>,
        max_iterations: usize,
    ) -> Self {
        Self {
            phase: AbstractForPhase::PushBody {
                carried,
                body,
                init_arg_count,
                results,
                iterations: 0,
            },
            body_stage,
            max_iterations,
            _phantom: PhantomData,
        }
    }
}

impl<E, C, V, L> Execute<E> for AbstractForCursor<V, L>
where
    V: Clone + ProductValue + kirin_interpreter::AbstractValue,
    L: Dialect,
    E: AbstractEnv<Value = V, Ext = CursorExt<C>>,
    E: Env<Mode = AbstractMode<C>>,
    AbstractBlockCursor<V, L>: Lift<C>,
    E::Error: From<InterpreterError>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<C>>, E::Error> {
        match std::mem::replace(&mut self.phase, AbstractForPhase::Done(PhantomData)) {
            AbstractForPhase::PushBody {
                carried,
                body,
                init_arg_count,
                results,
                iterations,
            } => {
                let mut block_args = Vec::with_capacity(1 + init_arg_count);
                block_args.push(V::bottom());
                if let Some(product) = carried.as_product() {
                    block_args.extend(product.iter().cloned());
                } else if init_arg_count > 0 {
                    block_args.push(carried.clone());
                }

                let cursor = AbstractBlockCursor::<V, L>::new(body, self.body_stage, block_args);
                self.phase = AbstractForPhase::WaitBody {
                    prev_carried: carried,
                    body,
                    init_arg_count,
                    results,
                    iterations,
                };
                Ok(Control::Ext(CursorExt::Push(cursor.lift())))
            }
            AbstractForPhase::WaitBody {
                prev_carried,
                body,
                init_arg_count,
                results,
                iterations,
            } => {
                let new_carried = inbox.unwrap_or_else(V::bottom);

                if new_carried.is_subseteq(&prev_carried) {
                    env.write_results(&results, new_carried)?;
                    return Ok(Control::Ext(CursorExt::Pop));
                }

                if iterations >= self.max_iterations {
                    let widened = prev_carried.join(&new_carried);
                    env.write_results(&results, widened)?;
                    return Ok(Control::Ext(CursorExt::Pop));
                }

                self.phase = AbstractForPhase::PushBody {
                    carried: new_carried,
                    body,
                    init_arg_count,
                    results,
                    iterations: iterations + 1,
                };
                Ok(Control::Advance)
            }
            AbstractForPhase::Done(_) => Err(E::Error::from(InterpreterError::UnhandledEffect(
                "AbstractForCursor executed after completion".into(),
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Composed cursor enums
// ---------------------------------------------------------------------------

pub enum SCFCursor<V, L: Dialect> {
    If(IfCursor<V, L>),
    For(ForCursor<V, L>),
}

impl<E, C, V, L> Execute<E> for SCFCursor<V, L>
where
    V: Clone + ProductValue + ForLoopValue,
    L: Dialect,
    E: Env<Mode = ConcreteMode<C>, Value = V, Ext = CursorExt<C>>,
    BlockCursor<V, L>: Lift<C>,
    E::Error: From<InterpreterError>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<C>>, E::Error> {
        match self {
            SCFCursor::If(c) => c.execute(env, inbox),
            SCFCursor::For(c) => c.execute(env, inbox),
        }
    }
}

pub enum AbstractSCFCursor<V, L: Dialect> {
    If(AbstractIfCursor<V, L>),
    For(AbstractForCursor<V, L>),
}

impl<E, C, V, L> Execute<E> for AbstractSCFCursor<V, L>
where
    V: Clone + ProductValue + kirin_interpreter::AbstractValue,
    L: Dialect,
    E: AbstractEnv<Value = V, Ext = CursorExt<C>>,
    E: Env<Mode = AbstractMode<C>>,
    AbstractBlockCursor<V, L>: Lift<C>,
    E::Error: From<InterpreterError>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<C>>, E::Error> {
        match self {
            AbstractSCFCursor::If(c) => c.execute(env, inbox),
            AbstractSCFCursor::For(c) => c.execute(env, inbox),
        }
    }
}
