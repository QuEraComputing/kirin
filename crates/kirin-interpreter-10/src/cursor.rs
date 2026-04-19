use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, SSAValue, StageInfo, Statement,
};

use crate::algebra::Lift;
use crate::control::{Control, CursorExt};
use crate::env::{AbstractEnv, AbstractMode, ConcreteMode, Env};
use crate::error::InterpreterError;
use crate::execute::Execute;
use crate::interpretable::Interpretable;

// ---------------------------------------------------------------------------
// Shared inner state — fields and traversal logic shared by both cursor types.
// Two separate outer types (`BlockCursor`, `AbstractBlockCursor`) are required
// for Rust coherence: having two `Execute<E>` impls on a single type is
// rejected (E0119) even when the `E::Mode` bounds are mutually exclusive.
// ---------------------------------------------------------------------------

pub(crate) struct BlockCursorState<V, L: Dialect> {
    pub block: Block,
    pub stage_id: CompileStage,
    pub current: Option<Statement>,
    /// Arguments to bind on the first `execute` call; taken once.
    pub init_args: Option<Vec<V>>,
    pub _phantom: PhantomData<L>,
}

impl<V, L: Dialect> BlockCursorState<V, L> {
    pub fn new(block: Block, stage_id: CompileStage, args: Vec<V>) -> Self {
        Self {
            block,
            stage_id,
            current: None,
            init_args: Some(args),
            _phantom: PhantomData,
        }
    }

    pub fn advance(&mut self, stage: &StageInfo<L>) {
        let Some(current) = self.current else {
            return;
        };
        self.current = if Some(current) == self.block.last_statement(stage) {
            None
        } else {
            (*current.next(stage)).or_else(|| self.block.terminator(stage))
        };
    }

    /// Bind block arguments on the first call. Leaves `current` pointing at
    /// the first statement.  Returns `Ok(true)` if init was needed, `Ok(false)`
    /// if already initialised.
    pub fn init<E>(&mut self, env: &mut E) -> Result<bool, E::Error>
    where
        E: Env<Value = V>,
        E::Stages: HasStageInfo<L>,
        V: Clone,
    {
        let Some(args) = self.init_args.take() else {
            return Ok(false);
        };

        let (ssa_keys, expected) = {
            let stage = env.require_stage::<L>(self.stage_id)?;
            let block_info = self.block.expect_info(stage);
            let expected = block_info.arguments.len();
            let ssa_keys: Vec<SSAValue> = block_info
                .arguments
                .iter()
                .map(|ba| SSAValue::from(*ba))
                .collect();
            (ssa_keys, expected)
        };

        if args.len() != expected {
            return Err(E::Error::from(InterpreterError::ArityMismatch {
                expected,
                got: args.len(),
            }));
        }

        for (ssa, val) in ssa_keys.into_iter().zip(args.iter()) {
            env.write_ssa(ssa, val.clone())?;
        }

        let stage = env.require_stage::<L>(self.stage_id)?;
        self.current = self.block.first_statement(stage);
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// BlockCursor — concrete mode linear block traversal
// ---------------------------------------------------------------------------

/// Linear cursor over statements in a single block for concrete interpreters.
///
/// Handles `Jump` internally by rebinding block args and looping.
/// All other effects (Call, Return, Yield, Ext) are returned to the driver.
pub struct BlockCursor<V, L: Dialect>(pub(crate) BlockCursorState<V, L>);

impl<V, L: Dialect> BlockCursor<V, L> {
    pub fn new(block: Block, stage_id: CompileStage, args: Vec<V>) -> Self {
        Self(BlockCursorState::new(block, stage_id, args))
    }

    pub fn block(&self) -> Block {
        self.0.block
    }

    pub fn stage_id(&self) -> CompileStage {
        self.0.stage_id
    }
}

impl<E, C, V, L> Execute<E> for BlockCursor<V, L>
where
    V: Clone,
    L: Dialect + Interpretable<E>,
    E: Env<Mode = ConcreteMode<C>, Value = V>,
    E::Stages: HasStageInfo<L>,
{
    fn execute(&mut self, env: &mut E, _inbox: Option<V>) -> Result<Control<V, E::Ext>, E::Error> {
        self.0.init(env)?;

        loop {
            let Some(stmt) = self.0.current else {
                return Err(E::Error::from(InterpreterError::NoCurrent));
            };

            let definition: L = {
                let stage = env.require_stage::<L>(self.0.stage_id)?;
                stmt.definition(stage).clone()
            };

            let effect = definition.eval(env)?;

            match effect {
                Control::Advance => {
                    let stage = env.require_stage::<L>(self.0.stage_id)?;
                    self.0.advance(stage);
                }
                Control::Jump(block, jump_args) => {
                    let (ssa_keys, first_stmt) = {
                        let stage = env.require_stage::<L>(self.0.stage_id)?;
                        let block_info = block.expect_info(stage);
                        let expected = block_info.arguments.len();
                        if jump_args.len() != expected {
                            return Err(E::Error::from(InterpreterError::ArityMismatch {
                                expected,
                                got: jump_args.len(),
                            }));
                        }
                        let ssa_keys: Vec<SSAValue> = block_info
                            .arguments
                            .iter()
                            .map(|ba| SSAValue::from(*ba))
                            .collect();
                        let first_stmt = block.first_statement(stage);
                        (ssa_keys, first_stmt)
                    };
                    self.0.block = block;
                    self.0.current = first_stmt;
                    for (ssa, val) in ssa_keys.into_iter().zip(jump_args.iter()) {
                        env.write_ssa(ssa, val.clone())?;
                    }
                }
                other => {
                    let stage = env.require_stage::<L>(self.0.stage_id)?;
                    self.0.advance(stage);
                    return Ok(other);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AbstractBlockCursor — abstract mode block traversal
// ---------------------------------------------------------------------------

/// Block cursor for abstract (worklist) interpreters.
///
/// On `Jump` or `Fork`, calls `env.enqueue_block()` and returns `Pop`.
/// This is a SEPARATE type from `BlockCursor` to satisfy Rust coherence (E0119).
///
/// # interpreter-10 fix vs interpreter-9
/// Abstract SCF cursors (`AbstractIfCursor`, `AbstractForCursor`) must use
/// `AbstractBlockCursor` — not `BlockCursor` — for their body execution.
/// In interpreter-9, they incorrectly created `BlockCursor` (the concrete type),
/// which cannot implement `Execute<E>` when `E: Env<Mode = AbstractMode<C>>`.
pub struct AbstractBlockCursor<V, L: Dialect>(pub(crate) BlockCursorState<V, L>);

impl<V, L: Dialect> AbstractBlockCursor<V, L> {
    pub fn new(block: Block, stage_id: CompileStage, args: Vec<V>) -> Self {
        Self(BlockCursorState::new(block, stage_id, args))
    }

    pub fn block(&self) -> Block {
        self.0.block
    }

    pub fn stage_id(&self) -> CompileStage {
        self.0.stage_id
    }
}

impl<E, C, V, L> Execute<E> for AbstractBlockCursor<V, L>
where
    V: Clone,
    L: Dialect + Interpretable<E>,
    E: AbstractEnv<Value = V, Ext = CursorExt<C>>,
    E: Env<Mode = AbstractMode<C>>,
    E::Stages: HasStageInfo<L>,
{
    fn execute(
        &mut self,
        env: &mut E,
        _inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<C>>, E::Error> {
        self.0.init(env)?;

        loop {
            let Some(stmt) = self.0.current else {
                return Ok(Control::Ext(CursorExt::Pop));
            };

            let definition: L = {
                let stage = env.require_stage::<L>(self.0.stage_id)?;
                stmt.definition(stage).clone()
            };

            let effect = definition.eval(env)?;

            match effect {
                Control::Advance => {
                    let stage = env.require_stage::<L>(self.0.stage_id)?;
                    self.0.advance(stage);
                }
                Control::Jump(block, args) => {
                    env.enqueue_block(block, args);
                    return Ok(Control::Ext(CursorExt::Pop));
                }
                Control::Fork(branches) => {
                    for (block, args) in branches {
                        env.enqueue_block(block, args);
                    }
                    return Ok(Control::Ext(CursorExt::Pop));
                }
                other => {
                    let stage = env.require_stage::<L>(self.0.stage_id)?;
                    self.0.advance(stage);
                    return Ok(other);
                }
            }
        }
    }
}

/// Marker that a `BlockCursor<V, L>` can be lifted into cursor type `C`.
pub trait HoldsBlockCursor<C, L: Dialect>: Sized + Lift<C> {}

impl<Cur, C, L: Dialect> HoldsBlockCursor<C, L> for Cur where Cur: Lift<C> {}

/// Marker that an `AbstractBlockCursor<V, L>` can be lifted into cursor type `C`.
pub trait HoldsAbstractBlockCursor<C, L: Dialect>: Sized + Lift<C> {}

impl<Cur, C, L: Dialect> HoldsAbstractBlockCursor<C, L> for Cur where Cur: Lift<C> {}
