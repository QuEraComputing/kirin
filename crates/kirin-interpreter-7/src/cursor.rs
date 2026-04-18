use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, SSAValue, StageInfo, Statement,
};

use crate::control::Control;
use crate::env::{ConcreteEnv, Interpretable};
use crate::error::InterpreterError;
use crate::lift::Lift;

/// Cursor execution trait for concrete execution.
///
/// Each cursor drives one unit of work: a block traversal, an SCF body phase,
/// etc. Returns `Control<E::Value, E::Ext>` — the language-level control effect.
///
/// Implementations are dialect-specific. The composed language cursor type
/// (e.g. `HighLevelCursor<V>`) implements `Execute<E>` by dispatching to each
/// variant's cursor.
///
/// # Note on `#[derive(ComposedCursor)]`
///
/// The composed cursor coproduct and its `Execute` impl are ~80 lines of
/// boilerplate. A `#[derive(ComposedCursor)]` macro will generate this in the
/// future.
// TODO: #[derive(ComposedCursor)] will generate the composed cursor coproduct
// and its Execute impl.
pub trait Execute<E: ConcreteEnv> {
    fn execute(&mut self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error>;
}

/// Linear cursor over statements in a single block.
///
/// Handles `Control::Advance` and `Control::Jump` internally (loops without
/// returning), so the driver loop only ever sees `Push`, `Pop`, `Yield`,
/// `Return`, or `Call` effects.
///
/// `L` is the dialect whose ops are interpreted. For a composed language, `L`
/// is the full language type, not a single dialect.
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

impl<E, L, V> Execute<E> for BlockCursor<V, L>
where
    V: Clone,
    L: Dialect + Interpretable<E, Effect = Control<V, E::Ext>>,
    E: ConcreteEnv<Value = V>,
    E::StageContainer: HasStageInfo<L>,
    E::Error: From<InterpreterError>,
{
    fn execute(&mut self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
        // First call: bind block arguments and set the initial statement pointer.
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

            // Clone the definition to release the stage borrow before calling interpret.
            let definition: L = {
                let stage: &StageInfo<L> = env
                    .stage_info_for::<L>(self.stage_id)
                    .ok_or_else(|| E::Error::from(InterpreterError::MissingEntry))?;
                stmt.definition(stage).clone()
            };

            let effect = definition.interpret(env)?;

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
                    // Advance the statement pointer past this statement before
                    // returning — the cursor continues from the next statement
                    // on the driver's next call.
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

/// Marker that a cursor type can lift a `BlockCursor<V, L>` into itself.
///
/// Used by `ConcreteInterp::enter_function` and `push_call_frame`.
pub trait HoldsBlockCursor<V: Clone, L: Dialect>: Lift<BlockCursor<V, L>> {}

impl<C, V: Clone, L: Dialect> HoldsBlockCursor<V, L> for C where C: Lift<BlockCursor<V, L>> {}
