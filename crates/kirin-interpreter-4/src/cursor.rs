use kirin_ir::{Block, Dialect, ResultValue, StageInfo, Statement};

/// Linear cursor over statements in a single block.
///
/// Generic over `V` so it can carry block arguments and result slots.
/// On the first call to [`Execute::execute`], any pending `args` are
/// bound to the block's SSA argument slots via
/// [`SingleStage::bind_block_args`].
#[derive(Debug, Clone)]
pub struct BlockCursor<V> {
    block: Block,
    current: Option<Statement>,
    results: Vec<ResultValue>,
    args: Option<Vec<V>>,
}

impl<V> BlockCursor<V> {
    pub fn new<L: Dialect>(
        stage: &StageInfo<L>,
        block: Block,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Self {
        Self {
            block,
            current: block.first_statement(stage),
            results,
            args: if args.is_empty() { None } else { Some(args) },
        }
    }

    pub fn block(&self) -> Block {
        self.block
    }

    pub fn current(&self) -> Option<Statement> {
        self.current
    }

    pub fn results(&self) -> &[ResultValue] {
        &self.results
    }

    pub fn is_exhausted(&self) -> bool {
        self.current.is_none()
    }

    pub fn take_args(&mut self) -> Option<Vec<V>> {
        self.args.take()
    }

    pub fn advance<L: Dialect>(&mut self, stage: &StageInfo<L>) {
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
// Execute<SingleStage<...>> for BlockCursor<V>
// ---------------------------------------------------------------------------

use crate::concrete::{Action, SingleStage};
use crate::error::InterpreterError;
use crate::execute::Execute;
use crate::lift::LiftInto;
use crate::traits::{Interpretable, Machine};

impl<'ir, L, V, M, C> Execute<SingleStage<'ir, L, V, M, C>> for BlockCursor<V>
where
    L: Dialect + Interpretable<SingleStage<'ir, L, V, M, C>>,
    <L as Interpretable<SingleStage<'ir, L, V, M, C>>>::Effect: LiftInto<Action<V, M::Effect, C>>,
    <L as Interpretable<SingleStage<'ir, L, V, M, C>>>::Error: Into<InterpreterError>,
    V: Clone,
    M: Machine<Error = InterpreterError>,
    C: crate::lift::Lift<BlockCursor<V>>,
{
    fn execute(
        &mut self,
        interp: &mut SingleStage<'ir, L, V, M, C>,
    ) -> Result<Action<V, M::Effect, C>, InterpreterError> {
        // Bind block arguments on first entry into this cursor.
        if let Some(args) = self.take_args() {
            interp.bind_block_args(self.block, &args)?;
        }

        loop {
            let Some(stmt) = self.current else {
                // Block exhausted without a structural effect — malformed IR.
                return Err(InterpreterError::NoCurrent);
            };

            let stage = interp.stage_info();
            let definition = stmt.definition(stage);
            let effect: Action<V, M::Effect, C> = definition
                .interpret(interp)
                .map_err(Into::into)?
                .lift_into();

            match &effect {
                Action::Advance => {
                    let stage = interp.stage_info();
                    self.advance(stage);
                    continue;
                }
                Action::Jump(block, args) => {
                    let block = *block;
                    let args = args.clone();
                    let stage = interp.stage_info();
                    self.current = block.first_statement(stage);
                    self.block = block;
                    interp.bind_block_args(block, &args)?;
                    continue;
                }
                _ => {
                    // Structural effect — advance past the current statement
                    // before returning control to the driver.
                    let stage = interp.stage_info();
                    self.advance(stage);
                    return Ok(effect);
                }
            }
        }
    }
}
