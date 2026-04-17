use std::marker::PhantomData;

use kirin_ir::{
    Block, Dialect, GetInfo, HasStageInfo, ResultValue, SSAValue, StageInfo, Statement,
};

/// Linear cursor over statements in a single block.
///
/// `L` is a phantom dialect type that selects which stage's IR to read when
/// traversing statements. `Execute<I>` uses `I::current_stage_info::<L>()` so
/// the same cursor implementation drives both `SingleStage<L>` and `MultiStage<S>`
/// as long as `I::StageInfo: HasStageInfo<L>`.
#[derive(Debug, Clone)]
pub struct BlockCursor<V, L: Dialect> {
    block: Block,
    current: Option<Statement>,
    results: Vec<ResultValue>,
    args: Option<Vec<V>>,
    _phantom: PhantomData<L>,
}

impl<V, L: Dialect> BlockCursor<V, L> {
    pub fn new(
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
            _phantom: PhantomData,
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
}

// ---------------------------------------------------------------------------
// Generic Execute<I> for BlockCursor<V, L>
// ---------------------------------------------------------------------------

use crate::effect::{IsAdvance, IsJump};
use crate::error::InterpreterError;
use crate::execute::Execute;
use crate::lift::LiftInto;
use crate::traits::{Interpretable, Interpreter, Machine, PipelineAccess, ValueStore};

impl<I, V, L> Execute<I> for BlockCursor<V, L>
where
    L: Dialect + Interpretable<I>,
    <L as Interpretable<I>>::Error: Into<<I as Machine>::Error>,
    <L as Interpretable<I>>::Effect: LiftInto<<I as Machine>::Effect>,
    I: Interpreter + ValueStore<Value = V>,
    <I as Machine>::Error: From<InterpreterError>,
    <I as Machine>::Effect: IsAdvance + IsJump<Value = V>,
    <I as PipelineAccess>::StageInfo: HasStageInfo<L>,
    V: Clone,
{
    fn execute(&mut self, interp: &mut I) -> Result<<I as Machine>::Effect, <I as Machine>::Error> {
        // Bind block arguments on first entry into this cursor.
        if let Some(args) = self.take_args() {
            let ssa_keys: Vec<SSAValue> = {
                let stage = interp
                    .current_stage_info::<L>()
                    .ok_or_else(|| <I as Machine>::Error::from(InterpreterError::MissingEntry))?;
                let block_info = self.block.expect_info(stage);
                let expected = block_info.arguments.len();
                if args.len() != expected {
                    return Err(<I as Machine>::Error::from(
                        InterpreterError::ArityMismatch {
                            expected,
                            got: args.len(),
                        },
                    ));
                }
                block_info
                    .arguments
                    .iter()
                    .map(|ba| SSAValue::from(*ba))
                    .collect()
            };
            for (ssa, value) in ssa_keys.into_iter().zip(args.iter()) {
                interp.write_ssa(ssa, value.clone())?;
            }
        }

        loop {
            let Some(stmt) = self.current else {
                return Err(<I as Machine>::Error::from(InterpreterError::NoCurrent));
            };

            // Clone the definition so we release the stage borrow before calling interpret.
            let definition: L = {
                let stage = interp
                    .current_stage_info::<L>()
                    .ok_or_else(|| <I as Machine>::Error::from(InterpreterError::MissingEntry))?;
                stmt.definition(stage).clone()
            };

            let effect: <I as Machine>::Effect = definition
                .interpret(interp)
                .map_err(|e| e.into())?
                .lift_into();

            if effect.is_advance() {
                let stage = interp
                    .current_stage_info::<L>()
                    .ok_or_else(|| <I as Machine>::Error::from(InterpreterError::MissingEntry))?;
                self.advance(stage);
                continue;
            }

            // Extract jump data (owned) before releasing borrow on effect.
            let jump_data = effect.as_jump().map(|(b, args)| (b, args.to_vec()));
            if let Some((block, args)) = jump_data {
                let first_stmt = {
                    let stage = interp.current_stage_info::<L>().ok_or_else(|| {
                        <I as Machine>::Error::from(InterpreterError::MissingEntry)
                    })?;
                    block.first_statement(stage)
                };
                self.current = first_stmt;
                self.block = block;

                let ssa_keys: Vec<SSAValue> = {
                    let stage = interp.current_stage_info::<L>().ok_or_else(|| {
                        <I as Machine>::Error::from(InterpreterError::MissingEntry)
                    })?;
                    let block_info = block.expect_info(stage);
                    let expected = block_info.arguments.len();
                    if args.len() != expected {
                        return Err(<I as Machine>::Error::from(
                            InterpreterError::ArityMismatch {
                                expected,
                                got: args.len(),
                            },
                        ));
                    }
                    block_info
                        .arguments
                        .iter()
                        .map(|ba| SSAValue::from(*ba))
                        .collect()
                };
                for (ssa, val) in ssa_keys.into_iter().zip(args.iter()) {
                    interp.write_ssa(ssa, val.clone())?;
                }
                continue;
            }

            // Structural effect — advance past the current statement before returning.
            {
                let stage = interp
                    .current_stage_info::<L>()
                    .ok_or_else(|| <I as Machine>::Error::from(InterpreterError::MissingEntry))?;
                self.advance(stage);
            }
            return Ok(effect);
        }
    }
}
