use std::marker::PhantomData;

use kirin_ir::{Block, Dialect, LiftFrom, Product, TryLift, TryLiftFrom};

use crate::{
    ConcreteBlockTransfer, Env, EnvIndex, Frame, FrameEffect, HasLocation, InterpreterError,
    Location, Position, ProjectOrSelf, StageAccess, StandardCompletion, StatementDispatch,
    Traversal,
};

use super::{BlockFrame, BlockTransferDispatch, StandardFrame};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionFrame<L, V, T = ConcreteBlockTransfer<V>> {
    pub location: Location,
    pub region: kirin_ir::Region,
    pub traversal: Traversal<Block>,
    pub env: EnvIndex,
    pub incoming_args: Product<V>,
    _marker: PhantomData<fn() -> (L, T)>,
}

impl<L, V, T> RegionFrame<L, V, T> {
    pub fn new(
        stage: kirin_ir::CompileStage,
        region: kirin_ir::Region,
        env: EnvIndex,
        incoming_args: Product<V>,
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

impl<L, V, T> HasLocation for RegionFrame<L, V, T> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<L, V, T> RegionFrame<L, V, T> {
    fn enter<I, F, C, E>(self, interp: &mut I) -> Result<FrameEffect<F, C>, E>
    where
        I: StageAccess<L, Error = E>,
        L: Dialect,
        F: TryLiftFrom<StandardFrame<L, V, T>>,
        E: From<<F as TryLiftFrom<StandardFrame<L, V, T>>>::Error>,
        V: Clone,
    {
        let first_block = {
            let stage = interp.stage_info(self.location.stage)?;
            self.region.blocks(stage).next()
        };
        match first_block {
            Some(block) => {
                let incoming_args = self.incoming_args.clone();
                self.push_block(block, incoming_args)
            }
            None => self
                .with_traversal(Traversal::Exit)
                .into_standard_frame()
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from),
        }
    }

    fn push_block<F, C, E>(
        self,
        block: Block,
        incoming_args: Product<V>,
    ) -> Result<FrameEffect<F, C>, E>
    where
        F: TryLiftFrom<StandardFrame<L, V, T>>,
        E: From<<F as TryLiftFrom<StandardFrame<L, V, T>>>::Error>,
    {
        let stage = self.location.stage;
        let env = self.env;
        let parent = self
            .with_traversal(Traversal::Active(block))
            .into_standard_frame()
            .try_lift()?;
        let child = BlockFrame::<L, V, T>::new(stage, block, env, incoming_args)
            .into_standard_frame()
            .try_lift()?;
        Ok(FrameEffect::Push { parent, child })
    }

    fn into_standard_frame(self) -> StandardFrame<L, V, T> {
        StandardFrame::Region(self)
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

impl<I, L, F, C, E, V, T> Frame<I, F, C, E> for RegionFrame<L, V, T>
where
    I: StageAccess<L, Error = E>
        + StatementDispatch<L, F, C, E, T>
        + BlockTransferDispatch<L, F, C, E, V, T>
        + Env<V, Error = E>,
    L: Dialect,
    F: TryLiftFrom<StandardFrame<L, V, T>>,
    C: TryLiftFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
    E: LiftFrom<InterpreterError>
        + From<<F as TryLiftFrom<StandardFrame<L, V, T>>>::Error>
        + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
    V: Clone,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match self.traversal {
            Traversal::Entry => self.enter(interp),
            Traversal::Active(_) => Err(E::lift_from(InterpreterError::Custom(
                "region frame is waiting for block completion",
            ))),
            Traversal::Exit => self.complete(),
        }
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Done)
    }

    fn resume(self, completion: C, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        match completion.project_or_self() {
            Ok(StandardCompletion::BlockDone) => {}
            Ok(completion) => return Ok(FrameEffect::Complete(C::try_lift_from(completion)?)),
            Err(completion) => return Ok(FrameEffect::Complete(completion)),
        }

        match self.traversal {
            Traversal::Active(_) => self
                .with_traversal(Traversal::Exit)
                .into_standard_frame()
                .try_lift()
                .map(FrameEffect::Continue)
                .map_err(E::from),
            _ => Err(E::lift_from(InterpreterError::ExpectedActiveBlock(
                self.location,
            ))),
        }
    }
}
