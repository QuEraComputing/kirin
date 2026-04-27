use std::marker::PhantomData;

use kirin_ir::{Block, Dialect, GetInfo, SSAValue, TryLiftFrom};

use crate::{
    ConcreteTransfer, Env, EnvIndex, Frame, FrameEffect, HasLocation, InterpreterError, Location,
    Position, StageAccess, StandardCompletion, StatementDispatch, StatementEffect, Traversal,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockFrame<L, V> {
    pub location: Location,
    pub block: Block,
    pub traversal: Traversal<kirin_ir::Statement>,
    pub env: EnvIndex,
    pub incoming_args: Vec<V>,
    _marker: PhantomData<fn() -> L>,
}

impl<L, V> BlockFrame<L, V> {
    pub fn new(
        stage: kirin_ir::CompileStage,
        block: Block,
        env: EnvIndex,
        incoming_args: Vec<V>,
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

impl<L, V> HasLocation for BlockFrame<L, V> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<L, V> BlockFrame<L, V> {
    fn bind_block_args<I, E>(&self, interp: &mut I) -> Result<(), E>
    where
        I: StageAccess<L, Error = E> + Env<V, Error = E>,
        L: Dialect,
        E: From<InterpreterError>,
        V: Clone,
    {
        let block_args = {
            let stage = interp.stage_info(self.location.stage)?;
            self.block.expect_info(stage).arguments.clone()
        };

        if block_args.len() != self.incoming_args.len() {
            return Err(InterpreterError::UnexpectedCompletion {
                location: self.location,
                completion: "block argument arity mismatch",
            }
            .into());
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
        F: From<BlockFrame<L, V>>,
        E: From<InterpreterError>,
        V: Clone,
    {
        self.bind_block_args(interp)?;
        let first_statement = {
            let stage = interp.stage_info(self.location.stage)?;
            self.block.first_statement(stage)
        };
        match first_statement {
            Some(statement) => Ok(FrameEffect::Continue(
                self.with_traversal(Traversal::Active(statement)).into(),
            )),
            None => Ok(FrameEffect::Continue(
                self.with_traversal(Traversal::Exit).into(),
            )),
        }
    }

    fn advance_after_active<I, F, C, E>(self, interp: &mut I) -> Result<FrameEffect<F, C>, E>
    where
        I: StageAccess<L, Error = E>,
        L: Dialect,
        F: From<BlockFrame<L, V>>,
        E: From<InterpreterError>,
    {
        let statement = match self.traversal {
            Traversal::Active(statement) => statement,
            _ => return Err(InterpreterError::ExpectedActiveStatement(self.location).into()),
        };
        let next_statement = {
            let stage = interp.stage_info(self.location.stage)?;
            *statement.next(stage)
        };
        match next_statement {
            Some(statement) => Ok(FrameEffect::Continue(
                self.with_traversal(Traversal::Active(statement)).into(),
            )),
            None => Ok(FrameEffect::Continue(
                self.with_traversal(Traversal::Exit).into(),
            )),
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

    fn jump<F, C>(self, target: Block, arguments: Vec<V>) -> FrameEffect<F, C>
    where
        F: From<BlockFrame<L, V>>,
    {
        FrameEffect::Continue(
            BlockFrame::<L, V>::new(self.location.stage, target, self.env, arguments).into(),
        )
    }
}

impl<I, L, F, C, E, V> Frame<I, F, C, E> for BlockFrame<L, V>
where
    I: StageAccess<L, Error = E>
        + StatementDispatch<L, F, C, E, ConcreteTransfer<V>>
        + Env<V, Error = E>,
    L: Dialect,
    F: From<BlockFrame<L, V>>,
    C: TryLiftFrom<StandardCompletion<V>>,
    E: From<InterpreterError> + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
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
                    StatementEffect::Transfer(ConcreteTransfer::Jump { target, arguments }) => {
                        Ok(self.jump(target, arguments))
                    }
                    StatementEffect::Push(child) => Ok(FrameEffect::Push {
                        parent: self.into(),
                        child,
                    }),
                    StatementEffect::Complete(completion) => Ok(FrameEffect::Complete(completion)),
                }
            }
            Traversal::Exit => self.complete(),
        }
    }

    fn resume(self, _completion: C, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        self.advance_after_active(interp)
    }
}
