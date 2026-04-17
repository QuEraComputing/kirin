use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, HasStageInfo, SSAValue, StageInfo, Statement,
};

use crate::concrete::ConcreteDomain;
use crate::effect::ControlFlow;
use crate::env::{Env, Interpretable};
use crate::error::InterpreterError;

/// The core cursor execution trait.
///
/// Each cursor drives one unit of work: a block traversal, an SCF body, etc.
/// The domain provides value read/write, stage info lookup, and sub-cursor creation.
pub trait Execute<D: Env> {
    fn execute(&mut self, domain: &mut D) -> Result<ControlFlow<D::Value, Boxed<D>>, D::Error>;
}

/// A type-erased owned cursor for heterogeneous cursor stacks.
///
/// `Boxed<D>` stores `Box<dyn Execute<D>>` where the concrete cursor type must be
/// `'static` (since `BlockCursor` stores IDs not references).
pub struct Boxed<D: Env>(pub Box<dyn Execute<D>>);

impl<D: Env> Boxed<D> {
    pub fn execute(&mut self, domain: &mut D) -> Result<ControlFlow<D::Value, Boxed<D>>, D::Error> {
        self.0.execute(domain)
    }
}

/// Linear cursor over statements in a single block.
///
/// Stores a `stage_id: CompileStage` (NOT a `&StageInfo<L>` reference) and looks
/// up the stage at execute time via `domain.stage_info_for::<L>()`. This makes
/// `BlockCursor` `'static` (no IR lifetime).
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

    pub fn current(&self) -> Option<Statement> {
        self.current
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
// Execute<D> for BlockCursor<V, L>
// ---------------------------------------------------------------------------

impl<D, L, V> Execute<D> for BlockCursor<V, L>
where
    L: Dialect + Interpretable<D>,
    D: ConcreteDomain,
    D: Env<Value = V, Effect = ControlFlow<V, Boxed<D>>>,
    D::StageContainer: HasStageInfo<L>,
    D::Error: From<InterpreterError>,
    V: Clone,
{
    fn execute(&mut self, domain: &mut D) -> Result<ControlFlow<V, Boxed<D>>, D::Error> {
        // Initialize: bind block args and set the first statement pointer.
        if let Some(args) = self.init_args.take() {
            let (ssa_keys, expected) = {
                let stage = domain
                    .stage_info_for::<L>(self.stage_id)
                    .ok_or_else(|| D::Error::from(InterpreterError::MissingEntry))?;
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
                return Err(D::Error::from(InterpreterError::ArityMismatch {
                    expected,
                    got: args.len(),
                }));
            }
            for (ssa, val) in ssa_keys.into_iter().zip(args.iter()) {
                domain.write_ssa(ssa, val.clone())?;
            }
            let stage = domain
                .stage_info_for::<L>(self.stage_id)
                .ok_or_else(|| D::Error::from(InterpreterError::MissingEntry))?;
            self.current = self.block.first_statement(stage);
        }

        loop {
            let Some(stmt) = self.current else {
                return Err(D::Error::from(InterpreterError::NoCurrent));
            };

            // Clone the definition so we release the stage borrow before calling interpret.
            let definition: L = {
                let stage = domain
                    .stage_info_for::<L>(self.stage_id)
                    .ok_or_else(|| D::Error::from(InterpreterError::MissingEntry))?;
                stmt.definition(stage).clone()
            };

            let effect = definition.interpret(domain)?;

            match effect {
                ControlFlow::Advance => {
                    let stage = domain
                        .stage_info_for::<L>(self.stage_id)
                        .ok_or_else(|| D::Error::from(InterpreterError::MissingEntry))?;
                    self.advance_stmt(stage);
                    // continue the loop
                }
                ControlFlow::Jump(block, args) => {
                    let (ssa_keys, first_stmt) = {
                        let stage = domain
                            .stage_info_for::<L>(self.stage_id)
                            .ok_or_else(|| D::Error::from(InterpreterError::MissingEntry))?;
                        let block_info = block.expect_info(stage);
                        let expected = block_info.arguments.len();
                        if args.len() != expected {
                            return Err(D::Error::from(InterpreterError::ArityMismatch {
                                expected,
                                got: args.len(),
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
                    for (ssa, val) in ssa_keys.into_iter().zip(args.iter()) {
                        domain.write_ssa(ssa, val.clone())?;
                    }
                    // continue the loop
                }
                other => {
                    // Structural effect: advance past current statement, then return.
                    {
                        let stage = domain
                            .stage_info_for::<L>(self.stage_id)
                            .ok_or_else(|| D::Error::from(InterpreterError::MissingEntry))?;
                        self.advance_stmt(stage);
                    }
                    return Ok(other);
                }
            }
        }
    }
}
