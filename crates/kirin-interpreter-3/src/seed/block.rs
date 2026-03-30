use kirin_ir::{Block, Dialect};
use smallvec::{SmallVec, smallvec};

use crate::{Effect, Execute, InterpError, InterpreterError, Machine, ProductValue};

use super::super::runtime::SingleStage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSeed<V> {
    block: Block,
    args: SmallVec<[V; 2]>,
}

impl<V> BlockSeed<V> {
    #[must_use]
    pub fn new(block: Block, args: impl Into<SmallVec<[V; 2]>>) -> Self {
        Self {
            block,
            args: args.into(),
        }
    }

    #[must_use]
    pub fn entry(block: Block) -> Self {
        Self {
            block,
            args: smallvec![],
        }
    }

    #[must_use]
    pub fn block(&self) -> Block {
        self.block
    }

    #[must_use]
    pub fn args(&self) -> &[V] {
        &self.args
    }

    #[must_use]
    pub fn into_parts(self) -> (Block, SmallVec<[V; 2]>) {
        (self.block, self.args)
    }
}

impl<V> From<Block> for BlockSeed<V> {
    fn from(block: Block) -> Self {
        Self::entry(block)
    }
}

impl<'ir, L, V, M, S> Execute<SingleStage<'ir, L, V, M, S>> for BlockSeed<V>
where
    L: Dialect + crate::Interpretable<SingleStage<'ir, L, V, M, S>>,
    V: Clone + ProductValue,
    M: Machine,
    M::Effect: crate::Lift<L::Effect>,
    M::Error: crate::Lift<L::Error>,
    S: kirin_ir::HasStageInfo<L>,
{
    type Output = Effect<V, M::Effect>;

    fn execute(
        self,
        interp: &mut SingleStage<'ir, L, V, M, S>,
    ) -> Result<Self::Output, <SingleStage<'ir, L, V, M, S> as Machine>::Error> {
        let original_cursor = interp.current_cursor()?;
        let block = self.block;
        let args = self.args;
        interp.enter_block(block, args)?;

        let result = execute_block(interp, block);
        let restore_result = interp.set_cursor(original_cursor);

        match (result, restore_result) {
            (Ok(effect), Ok(())) => Ok(effect),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error.into()),
            (Err(error), Err(_)) => Err(error),
        }
    }
}

fn execute_block<'ir, L, V, M, S>(
    interp: &mut SingleStage<'ir, L, V, M, S>,
    block: Block,
) -> Result<Effect<V, M::Effect>, InterpError<M::Error>>
where
    L: Dialect + crate::Interpretable<SingleStage<'ir, L, V, M, S>>,
    V: Clone + ProductValue,
    M: Machine,
    M::Effect: crate::Lift<L::Effect>,
    M::Error: crate::Lift<L::Error>,
    S: kirin_ir::HasStageInfo<L>,
{
    let terminator = block
        .terminator(interp.stage_info()?)
        .ok_or_else(InterpreterError::missing_terminator)?;

    loop {
        let current = interp.current_statement()?;
        let effect = interp.current_effect()?;

        if let Some(terminal) = split_terminal_effect(interp, effect)? {
            return Ok(terminal);
        }

        if current == terminator {
            return Err(InterpreterError::InvalidControl(
                "expected block terminator to produce a terminal effect",
            )
            .into());
        }
    }
}

fn split_terminal_effect<'ir, L, V, M, S>(
    interp: &mut SingleStage<'ir, L, V, M, S>,
    effect: Effect<V, M::Effect>,
) -> Result<Option<Effect<V, M::Effect>>, InterpError<M::Error>>
where
    L: Dialect + crate::Interpretable<SingleStage<'ir, L, V, M, S>>,
    V: Clone + ProductValue,
    M: Machine,
    M::Effect: crate::Lift<L::Effect>,
    M::Error: crate::Lift<L::Error>,
    S: kirin_ir::HasStageInfo<L>,
{
    match effect {
        Effect::Jump(..) | Effect::Return(..) | Effect::Yield(..) | Effect::Stop(..) => {
            Ok(Some(effect))
        }
        Effect::Seq(effects) => {
            let mut terminal = None;
            let len = effects.len();

            for (index, effect) in effects.into_iter().enumerate() {
                let effect = *effect;
                let split = split_terminal_effect(interp, effect)?;
                match split {
                    Some(found) => {
                        if index + 1 != len {
                            return Err(InterpreterError::InvalidControl(
                                "terminal effect must be final in a sequence",
                            )
                            .into());
                        }
                        terminal = Some(found);
                    }
                    None => {
                        if terminal.is_some() {
                            return Err(InterpreterError::InvalidControl(
                                "non-terminal effect cannot follow a terminal effect",
                            )
                            .into());
                        }
                    }
                }
            }

            Ok(terminal)
        }
        other => {
            interp.consume_effect(other)?;
            interp.clear_result();
            Ok(None)
        }
    }
}
