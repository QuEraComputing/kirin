use std::marker::PhantomData;

use kirin_ir::{Block, Dialect, TryLiftFrom};

use crate::{
    ConcreteTransfer, Env, EnvIndex, Frame, FrameEffect, HasLocation, InterpreterError, Location,
    Position, ProjectOrSelf, StageAccess, StandardCompletion, StatementDispatch, Traversal,
};

use super::BlockFrame;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionFrame<L, V> {
    pub location: Location,
    pub region: kirin_ir::Region,
    pub traversal: Traversal<Block>,
    pub env: EnvIndex,
    pub incoming_args: Vec<V>,
    _marker: PhantomData<fn() -> L>,
}

impl<L, V> RegionFrame<L, V> {
    pub fn new(
        stage: kirin_ir::CompileStage,
        region: kirin_ir::Region,
        env: EnvIndex,
        incoming_args: Vec<V>,
    ) -> Self {
        let traversal = Traversal::Entry;
        Self {
            location: Location::new(stage, Position::Region { region, traversal }),
            region,
            traversal,
            env,
            incoming_args,
            _marker: PhantomData,
        }
    }

    fn with_traversal(mut self, traversal: Traversal<Block>) -> Self {
        self.traversal = traversal;
        self.location.position = Position::Region {
            region: self.region,
            traversal,
        };
        self
    }
}

impl<L, V> HasLocation for RegionFrame<L, V> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<L, V> RegionFrame<L, V> {
    fn enter<I, F, C, E>(self, interp: &mut I) -> Result<FrameEffect<F, C>, E>
    where
        I: StageAccess<L, Error = E>,
        L: Dialect,
        F: From<RegionFrame<L, V>> + From<BlockFrame<L, V>>,
        V: Clone,
    {
        let first_block = {
            let stage = interp.stage_info(self.location.stage)?;
            self.region.blocks(stage).next()
        };
        match first_block {
            Some(block) => Ok(self.push_block(block)),
            None => Ok(FrameEffect::Continue(
                self.with_traversal(Traversal::Exit).into(),
            )),
        }
    }

    fn push_block<F, C>(self, block: Block) -> FrameEffect<F, C>
    where
        F: From<RegionFrame<L, V>> + From<BlockFrame<L, V>>,
        V: Clone,
    {
        let stage = self.location.stage;
        let env = self.env;
        let incoming_args = self.incoming_args.clone();
        let parent = self.with_traversal(Traversal::Active(block)).into();
        let child = BlockFrame::<L, V>::new(stage, block, env, incoming_args).into();
        FrameEffect::Push { parent, child }
    }

    fn next_block_after<I, E>(&self, block: Block, interp: &mut I) -> Result<Option<Block>, E>
    where
        I: StageAccess<L, Error = E>,
        L: Dialect,
    {
        let stage = interp.stage_info(self.location.stage)?;
        let mut blocks = self.region.blocks(stage);
        while let Some(candidate) = blocks.next() {
            if candidate == block {
                return Ok(blocks.next());
            }
        }
        Ok(None)
    }

    fn complete<F, C, E>(self) -> Result<FrameEffect<F, C>, E>
    where
        C: TryLiftFrom<StandardCompletion<V>>,
        E: From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
    {
        Ok(FrameEffect::Complete(C::try_lift_from(
            StandardCompletion::RegionDone,
        )?))
    }
}

impl<I, L, F, C, E, V> Frame<I, F, C, E> for RegionFrame<L, V>
where
    I: StageAccess<L, Error = E>
        + StatementDispatch<L, F, C, E, ConcreteTransfer<V>>
        + Env<V, Error = E>,
    L: Dialect,
    F: From<RegionFrame<L, V>> + From<BlockFrame<L, V>>,
    C: TryLiftFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
    E: From<InterpreterError> + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
    V: Clone,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self.traversal {
            Traversal::Entry => self.enter(interp),
            Traversal::Active(block) => Ok(self.push_block(block)),
            Traversal::Exit => self.complete(),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Done)
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match completion.project_or_self() {
            Ok(StandardCompletion::BlockDone) => {}
            Ok(completion) => return Ok(FrameEffect::Complete(C::try_lift_from(completion)?)),
            Err(completion) => return Ok(FrameEffect::Complete(completion)),
        }

        let active_block = match self.traversal {
            Traversal::Active(block) => block,
            _ => return Err(InterpreterError::ExpectedActiveBlock(self.location).into()),
        };
        match self.next_block_after(active_block, interp)? {
            Some(block) => Ok(self.push_block(block)),
            None => Ok(FrameEffect::Continue(
                self.with_traversal(Traversal::Exit).into(),
            )),
        }
    }
}
