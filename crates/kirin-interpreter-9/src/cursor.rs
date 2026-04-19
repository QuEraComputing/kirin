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

/// Linear cursor over statements in a single block.
///
/// - **Concrete mode** (`E::Mode = ConcreteMode<C>`): `Jump` is handled
///   internally by rebinding block args and looping. All other effects are
///   returned to the driver.
///
/// - **Abstract mode** (`E::Mode = AbstractMode<C>`): `Jump` and `Fork` call
///   `env.enqueue_block()` and return `Control::Ext(CursorExt::Pop)`. All
///   other effects are returned to the driver.
pub struct BlockCursor<V, L: Dialect> {
    block: Block,
    stage_id: CompileStage,
    current: Option<Statement>,
    init_args: Option<Vec<V>>,
    _phantom: PhantomData<L>,
}

impl<V, L: Dialect> BlockCursor<V, L> {
    pub fn new(block: Block, stage_id: CompileStage, args: Vec<V>) -> Self {
        Self {
            block,
            stage_id,
            current: None,
            init_args: Some(args),
            _phantom: PhantomData,
        }
    }

    pub fn block(&self) -> Block {
        self.block
    }

    pub fn stage_id(&self) -> CompileStage {
        self.stage_id
    }

    fn advance_stmt(&mut self, stage: &StageInfo<L>) {
        let Some(current) = self.current else {
            return;
        };
        self.current = if Some(current) == self.block.last_statement(stage) {
            None
        } else {
            (*current.next(stage)).or_else(|| self.block.terminator(stage))
        };
    }
}

// ---------------------------------------------------------------------------
// Concrete Execute impl (Mode = ConcreteMode<C>)
// ---------------------------------------------------------------------------

impl<E, C, V, L> Execute<E> for BlockCursor<V, L>
where
    V: Clone,
    L: Dialect + Interpretable<E>,
    E: Env<Mode = ConcreteMode<C>, Value = V>,
    E::Stages: HasStageInfo<L>,
    E::Error: From<InterpreterError>,
{
    fn execute(&mut self, env: &mut E, _inbox: Option<V>) -> Result<Control<V, E::Ext>, E::Error> {
        // First call: bind block arguments.
        if let Some(args) = self.init_args.take() {
            let (ssa_keys, expected) = {
                let stage: &StageInfo<L> = env
                    .stage_info_for::<L>(self.stage_id)
                    .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
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
            let stage: &StageInfo<L> = env
                .stage_info_for::<L>(self.stage_id)
                .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
            self.current = self.block.first_statement(stage);
        }

        loop {
            let Some(stmt) = self.current else {
                return Err(E::Error::from(InterpreterError::NoCurrent));
            };

            let definition: L = {
                let stage: &StageInfo<L> = env
                    .stage_info_for::<L>(self.stage_id)
                    .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
                stmt.definition(stage).clone()
            };

            let effect = definition.eval(env)?;

            match effect {
                Control::Advance => {
                    let stage: &StageInfo<L> = env
                        .stage_info_for::<L>(self.stage_id)
                        .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
                    self.advance_stmt(stage);
                }
                Control::Jump(block, jump_args) => {
                    let (ssa_keys, first_stmt) = {
                        let stage: &StageInfo<L> = env
                            .stage_info_for::<L>(self.stage_id)
                            .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
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
                    self.block = block;
                    self.current = first_stmt;
                    for (ssa, val) in ssa_keys.into_iter().zip(jump_args.iter()) {
                        env.write_ssa(ssa, val.clone())?;
                    }
                }
                other => {
                    {
                        let stage: &StageInfo<L> = env
                            .stage_info_for::<L>(self.stage_id)
                            .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
                        self.advance_stmt(stage);
                    }
                    return Ok(other);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AbstractBlockCursor — abstract mode block traversal
// ---------------------------------------------------------------------------

/// Block cursor for abstract (worklist) interpretation.
///
/// On `Jump` or `Fork`, calls `env.enqueue_block()` and returns `Pop`.
/// This is a SEPARATE type from `BlockCursor` to avoid coherence conflicts.
pub struct AbstractBlockCursor<V, L: Dialect> {
    block: Block,
    stage_id: CompileStage,
    current: Option<Statement>,
    init_args: Option<Vec<V>>,
    _phantom: PhantomData<L>,
}

impl<V, L: Dialect> AbstractBlockCursor<V, L> {
    pub fn new(block: Block, stage_id: CompileStage, args: Vec<V>) -> Self {
        Self {
            block,
            stage_id,
            current: None,
            init_args: Some(args),
            _phantom: PhantomData,
        }
    }

    pub fn block(&self) -> Block {
        self.block
    }

    fn advance_stmt(&mut self, stage: &StageInfo<L>) {
        let Some(current) = self.current else {
            return;
        };
        self.current = if Some(current) == self.block.last_statement(stage) {
            None
        } else {
            (*current.next(stage)).or_else(|| self.block.terminator(stage))
        };
    }
}

impl<E, C, V, L> Execute<E> for AbstractBlockCursor<V, L>
where
    V: Clone,
    L: Dialect + Interpretable<E>,
    E: AbstractEnv<Value = V, Ext = CursorExt<C>>,
    E: Env<Mode = AbstractMode<C>>,
    E::Stages: HasStageInfo<L>,
    E::Error: From<InterpreterError>,
{
    fn execute(
        &mut self,
        env: &mut E,
        _inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<C>>, E::Error> {
        if let Some(args) = self.init_args.take() {
            let (ssa_keys, expected) = {
                let stage: &StageInfo<L> = env
                    .stage_info_for::<L>(self.stage_id)
                    .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
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
            let stage: &StageInfo<L> = env
                .stage_info_for::<L>(self.stage_id)
                .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
            self.current = self.block.first_statement(stage);
        }

        loop {
            let Some(stmt) = self.current else {
                return Ok(Control::Ext(CursorExt::Pop));
            };

            let definition: L = {
                let stage: &StageInfo<L> = env
                    .stage_info_for::<L>(self.stage_id)
                    .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
                stmt.definition(stage).clone()
            };

            let effect = definition.eval(env)?;

            match effect {
                Control::Advance => {
                    let stage: &StageInfo<L> = env
                        .stage_info_for::<L>(self.stage_id)
                        .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
                    self.advance_stmt(stage);
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
                    {
                        let stage: &StageInfo<L> = env
                            .stage_info_for::<L>(self.stage_id)
                            .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
                        self.advance_stmt(stage);
                    }
                    return Ok(other);
                }
            }
        }
    }
}

/// Marker that a `BlockCursor<V, L>` can be lifted into cursor type `C`.
pub trait HoldsBlockCursor<C, L: Dialect>: Sized + Lift<C> {}

impl<Cur, C, L: Dialect> HoldsBlockCursor<C, L> for Cur where Cur: Lift<C> {}
