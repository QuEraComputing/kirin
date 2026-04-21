use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, SSAValue, StageInfo, Statement,
};

use crate::block_exec::{BlockExecEnv, JumpOutcome};
use crate::control::Control;
use crate::error::InterpreterError;
use crate::execute::Execute;
use crate::interpretable::Interpretable;

// ---------------------------------------------------------------------------
// BlockCursor — unified: one type, one Execute impl for all E: BlockExecEnv
// ---------------------------------------------------------------------------

pub struct BlockCursor<V, L: Dialect> {
    pub block: Block,
    pub stage_id: CompileStage,
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

    fn advance(&mut self, stage: &StageInfo<L>) {
        let Some(current) = self.current else { return };
        self.current = if Some(current) == self.block.last_statement(stage) {
            None
        } else {
            (*current.next(stage)).or_else(|| self.block.terminator(stage))
        };
    }
}

impl<E, V, L> Execute<E> for BlockCursor<V, L>
where
    V: Clone,
    L: Dialect + Interpretable<E>,
    E: BlockExecEnv<Value = V>,
    E::Stages: HasStageInfo<L>,
{
    fn execute(&mut self, env: &mut E, _inbox: Option<V>) -> Result<Control<V, E::Ext>, E::Error> {
        if let Some(args) = self.init_args.take() {
            let (ssa_keys, expected, first_stmt) = {
                let stage = env.require_stage::<L>(self.stage_id)?;
                let block_info = self.block.expect_info(stage);
                let expected = block_info.arguments.len();
                let ssa_keys: Vec<SSAValue> = block_info
                    .arguments
                    .iter()
                    .map(|ba| SSAValue::from(*ba))
                    .collect();
                (ssa_keys, expected, self.block.first_statement(stage))
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
            self.current = first_stmt;
        }

        loop {
            let Some(stmt) = self.current else {
                return Ok(env.exec_block_end());
            };

            let definition: L = {
                let stage = env.require_stage::<L>(self.stage_id)?;
                stmt.definition(stage).clone()
            };

            let effect = definition.eval(env)?;

            match effect {
                Control::Advance => {
                    let stage = env.require_stage::<L>(self.stage_id)?;
                    self.advance(stage);
                }
                Control::Jump(target, jump_args) => {
                    match env.exec_jump(target, jump_args.clone()) {
                        JumpOutcome::Rewound => {
                            let (ssa_keys, first_stmt) = {
                                let stage = env.require_stage::<L>(self.stage_id)?;
                                let block_info = target.expect_info(stage);
                                let ssa_keys: Vec<SSAValue> = block_info
                                    .arguments
                                    .iter()
                                    .map(|ba| SSAValue::from(*ba))
                                    .collect();
                                (ssa_keys, target.first_statement(stage))
                            };
                            self.block = target;
                            self.current = first_stmt;
                            for (ssa, val) in ssa_keys.into_iter().zip(jump_args.iter()) {
                                env.write_ssa(ssa, val.clone())?;
                            }
                        }
                        JumpOutcome::Done(ctrl) => return Ok(ctrl),
                    }
                }
                Control::Fork(branches) => {
                    return env.exec_fork(branches);
                }
                other => {
                    let stage = env.require_stage::<L>(self.stage_id)?;
                    self.advance(stage);
                    return Ok(other);
                }
            }
        }
    }
}
