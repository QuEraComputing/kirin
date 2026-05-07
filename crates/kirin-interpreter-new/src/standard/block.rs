use std::marker::PhantomData;

use kirin_ir::{Block, Dialect, GetInfo, LiftFrom, Product, SSAValue, TryLift, TryLiftFrom};

use crate::{
    ConcreteBlockTransfer, Env, EnvIndex, Frame, FrameEffect, HasLocation, InterpreterError,
    Location, Position, StageAccess, StandardCompletion, StatementDispatch, StatementEffect,
    Traversal,
};

use super::BlockTransferDispatch;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockFrame<L, V, T = ConcreteBlockTransfer<V>> {
    pub location: Location,
    pub block: Block,
    pub traversal: Traversal<kirin_ir::Statement>,
    pub env: EnvIndex,
    pub incoming_args: Product<V>,
    _marker: PhantomData<fn() -> (L, T)>,
}

impl<L, V, T> BlockFrame<L, V, T> {
    pub fn new(
        stage: kirin_ir::CompileStage,
        block: Block,
        env: EnvIndex,
        incoming_args: Product<V>,
    ) -> Self {
        let traversal = Traversal::Entry;
        Self {
            location: Location::new(stage, Position::Block { block, traversal }),
            block,
            traversal,
            env,
            incoming_args,
            _marker: PhantomData,
        }
    }

    fn with_traversal(mut self, traversal: Traversal<kirin_ir::Statement>) -> Self {
        self.traversal = traversal;
        self.location.position = Position::Block {
            block: self.block,
            traversal,
        };
        self
    }
}

impl<L, V, T> HasLocation for BlockFrame<L, V, T> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<L, V, T> BlockFrame<L, V, T> {
    fn bind_block_args<I, E>(&self, interp: &mut I) -> Result<(), E>
    where
        I: StageAccess<L, Error = E> + Env<V, Error = E>,
        L: Dialect,
        E: LiftFrom<InterpreterError>,
        V: Clone,
    {
        let block_args = {
            let stage = interp.stage_info(self.location.stage)?;
            self.block.expect_info(stage).arguments.clone()
        };

        if block_args.len() != self.incoming_args.len() {
            return Err(E::lift_from(InterpreterError::UnexpectedCompletion {
                location: self.location,
                completion: "block argument arity mismatch",
            }));
        }

        for (argument, value) in block_args
            .into_iter()
            .zip(self.incoming_args.iter().cloned())
        {
            interp.write(self.env, SSAValue::from(argument), value)?;
        }

        Ok(())
    }

    fn enter<I, F, C, E>(self, interp: &mut I) -> Result<FrameEffect<F, C>, E>
    where
        I: StageAccess<L, Error = E> + Env<V, Error = E>,
        L: Dialect,
        F: TryLiftFrom<BlockFrame<L, V, T>>,
        E: LiftFrom<InterpreterError> + From<<F as TryLiftFrom<BlockFrame<L, V, T>>>::Error>,
        V: Clone,
    {
        self.bind_block_args(interp)?;
        let first_statement = {
            let stage = interp.stage_info(self.location.stage)?;
            self.block.first_statement(stage)
        };
        match first_statement {
            Some(statement) => self
                .with_traversal(Traversal::Active(statement))
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from),
            None => self
                .with_traversal(Traversal::Exit)
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from),
        }
    }

    fn advance_after_active<I, F, C, E>(self, interp: &mut I) -> Result<FrameEffect<F, C>, E>
    where
        I: StageAccess<L, Error = E>,
        L: Dialect,
        F: TryLiftFrom<BlockFrame<L, V, T>>,
        E: LiftFrom<InterpreterError> + From<<F as TryLiftFrom<BlockFrame<L, V, T>>>::Error>,
    {
        let statement = match self.traversal {
            Traversal::Active(statement) => statement,
            _ => {
                return Err(E::lift_from(InterpreterError::ExpectedActiveStatement(
                    self.location,
                )));
            }
        };
        let next_statement = {
            let stage = interp.stage_info(self.location.stage)?;
            match *statement.next(stage) {
                Some(next) => Some(next),
                None if self.block.last_statement(stage) != Some(statement) => {
                    self.block.last_statement(stage)
                }
                None => None,
            }
        };
        match next_statement {
            Some(statement) => self
                .with_traversal(Traversal::Active(statement))
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from),
            None => self
                .with_traversal(Traversal::Exit)
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from),
        }
    }

    fn complete<F, C, E>(self) -> Result<FrameEffect<F, C>, E>
    where
        C: TryLiftFrom<StandardCompletion<V>>,
        E: From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
    {
        Ok(FrameEffect::Complete(C::try_lift_from(
            StandardCompletion::BlockDone,
        )?))
    }
}

impl<I, L, F, C, E, V, T> Frame<I, F, C, E> for BlockFrame<L, V, T>
where
    I: StageAccess<L, Error = E>
        + StatementDispatch<L, F, C, E, T>
        + BlockTransferDispatch<L, F, C, E, V, T>
        + Env<V, Error = E>,
    L: Dialect,
    F: TryLiftFrom<BlockFrame<L, V, T>>,
    C: TryLiftFrom<StandardCompletion<V>>,
    E: LiftFrom<InterpreterError>
        + From<<F as TryLiftFrom<BlockFrame<L, V, T>>>::Error>
        + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
    V: Clone,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self.traversal {
            Traversal::Entry => self.enter(interp),
            Traversal::Active(statement) => {
                let location = Location::new(
                    self.location.stage,
                    Position::Block {
                        block: self.block,
                        traversal: Traversal::Active(statement),
                    },
                );
                match interp.dispatch_statement(location, self.env)? {
                    StatementEffect::Done => self.advance_after_active(interp),
                    StatementEffect::Transfer(transfer) => {
                        interp.dispatch_block_transfer(self.location.stage, self.env, transfer)
                    }
                    StatementEffect::Push(child) => Ok(FrameEffect::Push {
                        parent: self.try_lift()?,
                        child,
                    }),
                    StatementEffect::Complete(completion) => Ok(FrameEffect::Complete(completion)),
                }
            }
            Traversal::Exit => self.complete(),
        }
    }

    fn resume_done(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        self.advance_after_active(interp)
    }

    fn resume(self, completion: C, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Complete(completion))
    }
}
